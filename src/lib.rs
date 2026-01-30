#![deny(clippy::all)]

use crate::dom::{Document, DocumentHolder};
use anyrender_vello::VelloWindowRenderer;
use blitz::{
    dom::Document as BlitzDocument,
    shell::{create_default_event_loop, BlitzApplication, BlitzShellProxy, WindowConfig},
};
use napi_derive::napi;
use std::{ops::Deref, time::Duration};
use winit::event_loop::{
    pump_events::{EventLoopExtPumpEvents, PumpStatus},
    EventLoop,
};

mod dom;

#[napi]
pub struct BlitzApp {
    event_loop: EventLoop,
    application: BlitzApplication<VelloWindowRenderer>,
}

#[napi]
pub struct PumpResult {
    pub r#continue: bool,
    pub exit: bool,
    pub code: Option<i32>,
}

#[napi]
impl BlitzApp {
    #[napi]
    pub fn create() -> Self {
        let event_loop = create_default_event_loop();
        let (proxy, receiver) = BlitzShellProxy::new(event_loop.create_proxy());
        // Create application
        let application = BlitzApplication::new(proxy, receiver);

        Self {
            event_loop,
            application,
        }
    }

    #[napi]
    pub fn open_window(&mut self, document: &Document) {
        document.doc.borrow_mut().resolve(0.0);
        self.application.add_window(WindowConfig::new(
            unsafe {
                Box::<DocumentHolder>::from_raw(
                    document.doc.borrow_mut().deref() as *const _ as *mut DocumentHolder
                )
            } as Box<dyn BlitzDocument>,
            VelloWindowRenderer::new(),
        ));
    }

    #[napi]
    pub fn pump_app_events(&mut self, millis: f64) -> PumpResult {
        match self.event_loop.pump_app_events(
            Some(Duration::from_millis(millis.round() as u64)),
            &mut self.application,
        ) {
            PumpStatus::Continue => PumpResult {
                r#continue: true,
                exit: false,
                code: None,
            },
            PumpStatus::Exit(code) => PumpResult {
                r#continue: false,
                exit: true,
                code: Some(code),
            },
        }
    }
}
