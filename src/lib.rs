#![deny(clippy::all)]

use crate::dom::{Document, DocumentHolder};
use anyrender_vello::VelloWindowRenderer;
use blitz::dom::Document as BlitzDocument;
use blitz::shell::{create_default_event_loop, BlitzApplication, BlitzShellEvent, WindowConfig};
use napi_derive::napi;
use std::ops::Deref;
use std::time::Duration;
use winit::event_loop::EventLoop;
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};

mod dom;

unsafe impl Send for Document {}
unsafe impl Sync for Document {}

#[napi]
pub struct BlitzApp {
    event_loop: EventLoop<BlitzShellEvent>,
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
        // Create application
        let application = BlitzApplication::new(event_loop.create_proxy());

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
