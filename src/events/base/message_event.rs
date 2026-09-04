//! The `MessageEvent` class — inherits from `Event`.

use napi::Result;
use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, proc::layer},
};

use super::{EventInit, EventLayer};

/// `dictionary MessageEventInit : EventInit`.
/// `source` and `ports` are not implemented yet.
#[napi(object)]
#[derive(Default)]
pub struct MessageEventInit {
    pub data: Option<Anything>,
    pub origin: Option<String>,
    pub last_event_id: Option<String>,
    pub bubbles: Option<bool>,
    pub cancelable: Option<bool>,
    pub composed: Option<bool>,
}

/// Own block of the `MessageEvent` class.
#[layer]
pub struct MessageEventLayer {
    data: Anything,
    origin: String,
    last_event_id: String,
}

#[layer(js_name = "MessageEvent")]
impl MessageEventLayer {
    #[layer(parent)]
    type Parent = EventLayer;

    /// `new MessageEvent(type, init?)` — `init` follows
    /// `dictionary MessageEventInit`.
    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<MessageEventInit>,
        sup: Super<EventLayer>,
    ) -> Result<Constructed<Self>> {
        let MessageEventInit {
            data,
            origin,
            last_event_id,
            bubbles,
            cancelable,
            composed,
        } = init.unwrap_or_default();
        let event_init = Some(EventInit {
            bubbles,
            cancelable,
            composed,
        });
        let done = sup.call(FnArgs::from((type_, event_init)))?;
        Ok(Constructed::new(
            done,
            Self {
                data: data.unwrap_or(Anything::Null),
                origin: origin.unwrap_or_default(),
                last_event_id: last_event_id.unwrap_or_default(),
            },
        ))
    }

    #[layer(getter)]
    fn data(&self) -> Anything {
        self.data.clone()
    }

    #[layer(getter)]
    fn origin(&self) -> String {
        self.origin.clone()
    }

    #[layer(getter)]
    fn last_event_id(&self) -> String {
        self.last_event_id.clone()
    }
}
