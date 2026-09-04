//! Event dispatch: Rust drives the three-phase chain walk over the
//! `#[layer]` event and node classes (`wintertc-events` + the DOM layers).
//!
//! When blitz produces a `DomEvent`, [`JsEventHandler`] receives the event
//! chain (root → target) and the raw event data. It:
//!
//! 1. Builds the most specific `Event`-derived layer instance for the
//!    payload (`crate::events::build_event`) — no JS event factory.
//! 2. Sets `event.target` to a lazy [`DispatchTarget`] (only wraps the
//!    target node when JS code actually reads `event.target`).
//! 3. Walks the chain in capture → target → bubble order. For each
//!    receiver it calls `wrap_node(node_id)` to get the JS Node object,
//!    writes `currentTarget` / `eventPhase` into the event's `EventLayer`
//!    state, and invokes `EventTargetLayer::dispatch_event` directly.
//! 4. Reads back `preventDefault` / `stopPropagation` from the
//!    `EventLayer` state and writes them to blitz's `EventState`.

use std::rc::{Rc, Weak};

use crate::{
    dom::shared::{SharedDocument, wrap_node},
    events::base::{DispatchTarget, EventLayer, EventTargetLayer},
};
use blitz::{
    dom::{Document as BlitzDocument, EventHandler, NodeData, NodeId},
    traits::events::{DomEvent, EventState},
};
use napi::{Env, Result, bindgen_prelude::Object};
use napi_helpers::inherits::{LayerRef, with_own, with_own_mut};

const CAPTURING_PHASE: u32 = 1;
const AT_TARGET: u32 = 2;
const BUBBLING_PHASE: u32 = 3;

/// `EventHandler` impl that drives the three-phase dispatch from Rust.
/// Holds a `Weak<SharedDocument>` so it doesn't conflict with `&mut self`
/// in `EventDriver::new(self, handler)`.
pub struct JsEventHandler {
    pub doc: Weak<SharedDocument>,
}

/// Normalize the event target: if it's an `AnonymousBlock` (blitz internal
/// layout node), walk up to the first non-anonymous ancestor.
fn normalize_event_target(doc: &dyn BlitzDocument, target: NodeId) -> NodeId {
    let inner = doc.inner();
    let mut node_id = target;
    while let Some(node) = inner.get_node(node_id) {
        match node.data {
            NodeData::AnonymousBlock(_) => {
                let Some(parent_id) = node.parent else {
                    return target;
                };
                node_id = parent_id;
            }
            NodeData::Element(_) => return node_id,
            _ => return target,
        }
    }
    target
}

/// Check if a node is an `AnonymousBlock` (blitz internal layout node).
fn is_anonymous(doc: &dyn BlitzDocument, node_id: NodeId) -> bool {
    doc.inner()
        .get_node(node_id)
        .map(|n| matches!(n.data, NodeData::AnonymousBlock(_)))
        .unwrap_or(false)
}

impl EventHandler for JsEventHandler {
    fn handle_event(
        &mut self,
        chain: &[NodeId],
        event: &mut DomEvent,
        doc: &mut dyn BlitzDocument,
        event_state: &mut EventState,
    ) {
        let Some(shared_doc) = self.doc.upgrade() else {
            return;
        };
        let Some(env) = shared_doc.env() else {
            return;
        };

        // The dispatch pipeline is best-effort: a napi boundary failure is
        // logged and dropped, the event is simply not delivered further.
        if let Err(e) = self.dispatch(chain, event, doc, event_state, &shared_doc, &env) {
            eprintln!("napi-blitz-dom: event dispatch failed: {e}");
        }
    }
}

impl JsEventHandler {
    /// Run the full event dispatch pipeline: build the `Event` layer
    /// instance, walk the chain in capture → target → bubble order, reset
    /// transient dispatch state, and write the resulting flags back to
    /// blitz's `EventState`.
    fn dispatch(
        &mut self,
        chain: &[NodeId],
        event: &mut DomEvent,
        doc: &mut dyn BlitzDocument,
        event_state: &mut EventState,
        shared_doc: &Rc<SharedDocument>,
        env: &Env,
    ) -> Result<()> {
        // 1. Build the event layer instance for the payload.
        let event_obj = crate::events::build_event(
            env,
            event.name(),
            &event.data,
            event.bubbles,
            event.cancelable,
        )?;

        // 2. Normalize target (skip AnonymousBlock).
        let target_nid = normalize_event_target(doc, event.target);

        // 3. Set lazy target on the event's dispatch state.
        {
            let doc_clone = shared_doc.clone();
            with_own_mut::<EventLayer, _>(&event_obj, |d| {
                d.state_mut().target = DispatchTarget::from_callable(Box::new(move |env| {
                    let node = wrap_node(&doc_clone, env, target_nid)?;
                    LayerRef::new(&node, env)
                }));
            })?;
        }

        // 4. Filter anonymous nodes from the chain.
        let clean_chain: Vec<NodeId> = chain
            .iter()
            .copied()
            .filter(|&nid| !is_anonymous(doc, nid))
            .collect();

        let mut propagation_stopped = false;

        // 5. Capture phase (root → target's parent).
        for &nid in clean_chain.iter().skip(1).rev() {
            if propagation_stopped {
                break;
            }
            propagation_stopped =
                self.dispatch_to_node(nid, &event_obj, CAPTURING_PHASE, shared_doc, env);
        }

        // 6. Target phase.
        if !propagation_stopped {
            propagation_stopped =
                self.dispatch_to_node(target_nid, &event_obj, AT_TARGET, shared_doc, env);
        }

        // 7. Bubble phase (target's parent → root).
        if event.bubbles && !propagation_stopped {
            for &nid in clean_chain.iter().skip(1) {
                if propagation_stopped {
                    break;
                }
                propagation_stopped =
                    self.dispatch_to_node(nid, &event_obj, BUBBLING_PHASE, shared_doc, env);
            }
        }

        // 8. Reset transient dispatch state: currentTarget → none,
        //    eventPhase → NONE (0). Per DOM spec, after dispatch ends
        //    these values are cleared so async callbacks see null.
        with_own_mut::<EventLayer, _>(&event_obj, |d| {
            let s = d.state_mut();
            s.current_target = DispatchTarget::None;
            s.phase = 0;
        })?;

        // 9. Read back flags and write to blitz EventState.
        let (canceled, stopped) = with_own::<EventLayer, _>(&event_obj, |d| {
            let s = d.state_ref();
            (s.canceled, s.stop_propagation || propagation_stopped)
        })?;
        if canceled {
            event_state.prevent_default();
        }
        if stopped {
            event_state.stop_propagation();
        }

        // 10. Sweep stale cache entries periodically.
        shared_doc.node_cache_mut().sweep(env);

        Ok(())
    }

    /// Dispatch the event to a single node. Returns `true` if propagation
    /// was stopped (stopPropagation / stopImmediatePropagation).
    fn dispatch_to_node(
        &self,
        node_id: NodeId,
        event: &Object,
        phase: u32,
        shared_doc: &Rc<SharedDocument>,
        env: &Env,
    ) -> bool {
        // 1. Wrap the node (NodeCache lookup or create).
        let node = match wrap_node(shared_doc, env, node_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("napi-blitz-dom: wrap_node failed for node {node_id}: {e}");
                return false;
            }
        };

        // 2. Set lazy currentTarget + eventPhase on the event state.
        {
            let doc_clone = shared_doc.clone();
            let _ = with_own_mut::<EventLayer, _>(event, |d| {
                let s = d.state_mut();
                s.phase = phase;
                s.current_target = DispatchTarget::from_callable(Box::new(move |env| {
                    let node = wrap_node(&doc_clone, env, node_id)?;
                    LayerRef::new(&node, env)
                }));
            });
        }

        // 3. Invoke the receiver's listener dispatch (`EventTargetLayer`
        //    slot on its chain), then read the event flags.
        if let Err(e) = with_own::<EventTargetLayer, _>(&node, |d| d.dispatch_event(env, *event)) {
            eprintln!("napi-blitz-dom: dispatch_event failed on node {node_id}: {e}");
            return false;
        }
        with_own::<EventLayer, _>(event, |d| {
            let s = d.state_ref();
            s.stop_propagation || s.stop_immediate
        })
        .unwrap_or(false)
    }
}
