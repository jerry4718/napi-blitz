use blitz::dom::{
    ns, Attribute as BlitzAttribute, BaseDocument, Document as BlitzDocument, DocumentConfig,
    DocumentMutator, EventDriver, EventHandler, LocalName, Node as BlitzNode, NodeData, QualName,
    DEFAULT_CSS,
};
use blitz::html::{DocumentHtmlParser, HtmlProvider};
use blitz::traits::events::{DomEvent, DomEventData, DomEventKind, EventState, UiEvent};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi::Env;
use napi_derive::napi;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;

#[napi]
pub struct Document {
    pub(crate) env: Env,
    pub(crate) doc: Rc<RefCell<DocumentHolder>>,
    pub(crate) nodes: HashMap<usize, Reference<Node>>,
}

pub struct DocumentHolder {
    pub(crate) base: BaseDocument,
    pub(crate) event_handler: SimpleEventHandler,
}

impl Deref for DocumentHolder {
    type Target = BaseDocument;
    fn deref(&self) -> &BaseDocument {
        &self.base
    }
}

impl DerefMut for DocumentHolder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Default, Clone)]
pub struct SimpleEventHandler {
    pub(crate) listeners: Vec<(
        usize,
        DomEventKind,
        Rc<RefCell<dyn FnMut(usize, &[usize], &mut DomEvent, &mut DocumentMutator)>>,
    )>,
}

impl EventHandler for SimpleEventHandler {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut DomEvent,
        mutr: &mut DocumentMutator<'_>,
        _event_state: &mut EventState,
    ) {
        for (node_id, kind, handler) in &self.listeners {
            if !matches!(kind, DomEventKind::MouseMove)
                && !matches!(event.data, DomEventData::MouseMove(_))
            {
                println!("Handling event {:?} from {}", event, node_id);
            }
            if event.target != *node_id {
                continue;
            }
            if event.data.kind() != *kind {
                continue;
            }
            println!("matched event ========================================================");
            handler.borrow_mut()(*node_id, chain, event, mutr)
        }
    }
}

impl BlitzDocument for DocumentHolder {
    fn handle_ui_event(&mut self, event: UiEvent) {
        if !matches!(event, UiEvent::MouseMove(_)) {
            println!("handle ui event: {:?}", event);
        }
        EventDriver::new(self.base.mutate(), self.event_handler.clone()).handle_ui_event(event);
    }

    /*fn poll(&mut self, task_context: Option<TaskContext>) -> bool {
        todo!()
    }*/

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /*fn id(&self) -> usize {
        todo!()
    }*/
}

#[napi]
pub struct Node {
    pub(crate) id: usize,
    pub(crate) doc: Rc<RefCell<DocumentHolder>>,
    pub(crate) props: HashMap<String, ObjectRef>,
}

#[napi(object)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

static BASE_HTML: &str = r#"<!DOCTYPE html>
<html>
    <head></head>
    <body id="body"></body>
</html>
"#;

impl Document {
    pub(crate) fn construct(env: Env, html: String) -> Document {
        let config = DocumentConfig::default();

        let mut base = BaseDocument::new(DocumentConfig {
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ua_stylesheets: Some(vec![DEFAULT_CSS.to_string()]),
            ..config
        });

        let mut mutr = base.mutate();
        DocumentHtmlParser::parse_into_mutator(&mut mutr, html.as_str());
        drop(mutr);
        base.resolve(0.0);

        let event_handler = SimpleEventHandler::default();

        let doc = Rc::new(RefCell::new(DocumentHolder {
            base,
            event_handler,
        }));

        Document {
            env,
            doc,
            nodes: Default::default(),
        }
    }
}

#[napi]
impl Document {
    #[napi(constructor)]
    pub fn new(env: Env, html: Option<String>) -> Document {
        Self::construct(env, html.unwrap_or(BASE_HTML.to_string()))
    }
}

#[napi]
impl Document {
    fn node_reference(&mut self, node_id: usize) -> Result<Reference<Node>> {
        let node = Node {
            id: node_id,
            doc: Rc::clone(&self.doc),
            props: Default::default(),
        };

        let node_reference = node.into_reference(self.env)?;
        self.nodes.insert(node_id, node_reference.clone(self.env)?);

        Ok(node_reference)
    }

    #[napi]
    pub fn get_node(&mut self, id: f64) -> Result<Option<Reference<Node>>> {
        let id = id as usize;
        if let Some(node_reference) = self.nodes.get(&id) {
            return match Reference::clone(node_reference, self.env) {
                Err(err) => Err(err),
                Ok(node_reference) => Ok(Some(node_reference)),
            };
        }

        let Some(id) = self.doc.borrow().get_node(id) else {
            return Ok(None);
        };

        self.node_reference(id)
            .map(|node_reference| Some(node_reference))
    }

    #[napi]
    pub fn deep_clone_node(&mut self, node: Reference<Node>) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().deep_clone_node(node.id);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_element(
        &mut self,
        name: String,
        attrs: Vec<Attribute>,
    ) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_element(name, attrs);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_text_node(&mut self, text: String) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_text_node(text);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_comment_node(&mut self) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_comment_node();
        self.node_reference(id)
    }

    #[napi]
    pub fn create_comment_node_with_content(&mut self, content: String) -> Result<Reference<Node>> {
        let id = self
            .doc
            .borrow_mut()
            .create_comment_node_with_content(content);
        self.node_reference(id)
    }

    #[napi]
    pub fn insert(
        &mut self,
        node: Option<Reference<Node>>,
        parent: Option<Reference<Node>>,
        anchor: Option<Reference<Node>>,
    ) {
        self.doc.borrow_mut().insert(
            node.map(|node| node.id),
            parent.map(|node| node.id),
            anchor.map(|node| node.id),
        );
    }

    #[napi]
    pub fn next_sibling(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().next_sibling(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn previous_sibling(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().previous_sibling(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn parent_node(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().parent_node(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn patch_prop(&mut self, node: Reference<Node>, name: String, value: String) {
        self.doc.borrow_mut().patch_prop(node.id, name, value);
    }

    #[napi]
    pub fn set_style_property(&mut self, node: Reference<Node>, name: String, value: String) {
        self.doc
            .borrow_mut()
            .set_style_property(node.id, name, value);
    }

    #[napi]
    pub fn query_selector(&mut self, selector: String) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().query_selector(selector)?;
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn remove(&mut self, node: Reference<Node>) {
        self.doc.borrow_mut().remove(node.id);
    }

    #[napi]
    pub fn set_element_text(&mut self, node: Reference<Node>, text: String) {
        self.doc.borrow_mut().set_element_text(node.id, text);
    }

    #[napi]
    pub fn set_text(&mut self, node: Reference<Node>, text: String) {
        self.doc.borrow_mut().set_text(node.id, text);
    }
}

impl DocumentHolder {
    pub fn get_node(&self, id: usize) -> Option<usize> {
        let node = self.base.get_node(id).unwrap();

        Some(node.id)
    }

    pub fn create_element(&mut self, name: String, attrs: Vec<Attribute>) -> usize {
        self.base.mutate().create_element(
            QualName::new(None, ns!(), LocalName::from(name.as_str())),
            attrs
                .iter()
                .map(|attr| BlitzAttribute {
                    name: QualName::new(None, ns!(), LocalName::from(attr.name.as_str())),
                    value: attr.value.clone(),
                })
                .collect(),
        )
    }

    pub fn deep_clone_node(&mut self, node_id: usize) -> usize {
        self.base.mutate().deep_clone_node(node_id)
    }

    pub fn create_text_node(&mut self, text: String) -> usize {
        self.base.mutate().create_text_node(&text)
    }

    pub fn create_comment_node(&mut self) -> usize {
        self.base.mutate().create_comment_node()
    }

    pub fn create_comment_node_with_content(&mut self, content: String) -> usize {
        let id = self.create_comment_node();

        self.set_element_text(id, content);

        id
    }

    pub fn insert(
        &mut self,
        node_id: Option<usize>,
        parent_id: Option<usize>,
        anchor_id: Option<usize>,
    ) {
        match (node_id, parent_id, anchor_id) {
            (None, _, _) | (_, None, None) => return,
            (Some(node), _, Some(anchor)) => {
                self.base.mutate().insert_nodes_after(anchor, &[node]);
            }
            (Some(node), Some(parent), None) => {
                self.base.mutate().append_children(parent, &[node]);
            }
        }
    }

    pub fn next_sibling(&mut self, node_id: usize) -> Option<usize> {
        self.base
            .get_node(node_id)
            .and_then(|node| node.forward(1).map(|node| node.id))
    }

    pub fn previous_sibling(&mut self, node_id: usize) -> Option<usize> {
        self.base
            .get_node(node_id)
            .and_then(|node| node.backward(1).map(|node| node.id))
    }

    pub fn parent_node(&mut self, node_id: usize) -> Option<usize> {
        self.base.mutate().parent_id(node_id)
    }

    pub fn patch_prop(&mut self, node_id: usize, name: String, value: String) {
        self.base.mutate().set_attribute(
            node_id,
            QualName::new(None, ns!(), LocalName::from(name.as_str())),
            &value,
        );
    }

    pub fn set_style_property(&mut self, node_id: usize, name: String, value: String) {
        self.base
            .mutate()
            .set_style_property(node_id, &name, &value);
    }

    pub fn query_selector(&mut self, selector: String) -> Result<Option<usize>> {
        self.base
            .query_selector(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector error: {:?}", err)))
    }

    pub fn remove(&mut self, node_id: usize) {
        self.base.mutate().remove_node(node_id);
    }

    pub fn set_element_text(&mut self, node_id: usize, text: String) {
        let Some(node) = self.base.get_node_mut(node_id) else {
            return;
        };

        let children = node.children.iter().map(|child| *child).collect::<Vec<_>>();

        let mut found = false;
        for child in children {
            if !self.base.get_node_mut(child).unwrap().is_text_node() {
                continue;
            }
            if found {
                self.base.mutate().remove_node(child);
                continue;
            }
            found = true;
            self.base.mutate().set_node_text(child, &text);
        }

        if !found {
            let text_node_id = self.create_text_node(text);
            self.insert(Some(text_node_id), Some(node_id), None);
        }
    }

    pub fn set_text(&mut self, node_id: usize, text: String) {
        if let Some(data) = self
            .base
            .get_node_mut(node_id)
            .and_then(|node| node.text_data_mut())
        {
            data.content = text
        }
    }

    pub fn add_event_listener<F>(&mut self, node: usize, kind: DomEventKind, event_handler: F)
    where
        F: FnMut(usize, &[usize], &mut DomEvent, &mut DocumentMutator) + 'static,
    {
        self.event_handler
            .listeners
            .push((node, kind, Rc::new(RefCell::new(event_handler))));
    }
}

#[napi]
impl Node {
    #[napi]
    pub fn print_tree(&mut self, level: Option<f64>) {
        let Some(level) = level else { return };
        self.doc
            .borrow()
            .base
            .get_node(self.id)
            .unwrap()
            .print_tree(level.round() as usize);
    }

    #[napi]
    pub fn self_prop(&mut self, name: String, value: ObjectRef) {
        self.props.insert(name, value);
    }

    #[napi]
    pub fn add_event_listener(
        &mut self,
        env: Env,
        event_type: String,
        handler: FunctionRef<(), ()>,
    ) {
        let event_kind = match event_type.as_str() {
            "click" => DomEventKind::Click,
            "mousemove" => DomEventKind::MouseMove,
            "mousedown" => DomEventKind::MouseDown,
            "mouseup" => DomEventKind::MouseUp,
            "keypress" => DomEventKind::KeyPress,
            "keydown" => DomEventKind::KeyDown,
            "keyup" => DomEventKind::KeyUp,
            "input" => DomEventKind::Input,
            _ => return,
        };
        let handler = handler
            .borrow_back(&env)
            .unwrap()
            .build_threadsafe_function()
            .build()
            .unwrap();

        self.doc.borrow_mut().add_event_listener(
            self.id,
            event_kind,
            move |node_id: usize,
                  chain_id: &[usize],
                  event: &mut DomEvent,
                  mutr: &mut DocumentMutator| {
                print!("event: {:?}, on: {}", event, node_id);
                handler.call((), ThreadsafeFunctionCallMode::NonBlocking);
            },
        );
    }
}
