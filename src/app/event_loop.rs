use blitz::shell::{ControlFlow, EventLoopProxy, Window};
use std::{
    fmt::{Debug, Formatter},
    ptr::NonNull,
};
use winit::{
    cursor::{CustomCursor, CustomCursorSource},
    error::RequestError,
    event_loop::{ActiveEventLoop, DeviceEvents, OwnedDisplayHandle},
    monitor::MonitorHandle,
    raw_window_handle::HasDisplayHandle,
    window::{Theme, WindowAttributes},
};

pub(crate) struct EventLoopBox {
    pub(crate) active: NonNull<dyn ActiveEventLoop>,
}

impl EventLoopBox {
    pub(crate) fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let ptr = event_loop as *const dyn ActiveEventLoop as *mut dyn ActiveEventLoop;
        Self {
            active: NonNull::new(ptr).unwrap(),
        }
    }

    fn inner(&self) -> &dyn ActiveEventLoop {
        // SAFETY: the borrowed object outlives the reference's scope, so the
        // pointer cannot dangle.
        unsafe { self.active.as_ref() }
    }
}

impl Debug for EventLoopBox {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner().fmt(f)
    }
}

impl ActiveEventLoop for EventLoopBox {
    fn create_proxy(&self) -> EventLoopProxy {
        self.inner().create_proxy()
    }

    fn create_window(
        &self,
        window_attributes: WindowAttributes,
    ) -> Result<Box<dyn Window>, RequestError> {
        let visible = window_attributes.visible;

        let with_invisible = window_attributes.with_visible(false);

        let window = self.inner().create_window(with_invisible)?;

        window.set_visible(visible);

        Ok(window)
    }

    fn create_custom_cursor(
        &self,
        custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        self.inner().create_custom_cursor(custom_cursor)
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        self.inner().available_monitors()
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.inner().primary_monitor()
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.inner().listen_device_events(allowed)
    }

    fn system_theme(&self) -> Option<Theme> {
        self.inner().system_theme()
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.inner().set_control_flow(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.inner().control_flow()
    }

    fn exit(&self) {
        self.inner().exit()
    }

    fn exiting(&self) -> bool {
        self.inner().exiting()
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.inner().owned_display_handle()
    }

    fn rwh_06_handle(&self) -> &dyn HasDisplayHandle {
        self.inner().rwh_06_handle()
    }
}
