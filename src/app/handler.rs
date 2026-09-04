//! `AppHandler`: the `winit::ApplicationHandler` adapter.
//!
//! Routes winit callbacks to `Lifecycle`; every lifecycle decision -
//! opens, closes, teardowns, the synthetic exit - lives there. This type
//! only decides *where* a callback goes: lifecycle events to
//! `Lifecycle`, window events to the owning `View`.

use std::rc::Rc;
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId as WinitWindowId,
};

use crate::app::{event_loop::EventLoopBox, lifecycle::Lifecycle};

pub struct AppHandler {
    pub lifecycle: Rc<Lifecycle>,
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        {
            let state = self.lifecycle.state();
            for entry in state.windows.values() {
                entry.view.borrow_mut().resume();
            }
        }
        self.lifecycle
            .drain_opening_windows(&EventLoopBox::new(event_loop), Rc::clone(&self.lifecycle));
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.lifecycle
            .drain_opening_windows(&EventLoopBox::new(event_loop), Rc::clone(&self.lifecycle));
        self.lifecycle
            .drain_shell_events(&EventLoopBox::new(event_loop));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.lifecycle
                .close_from_os(window_id, &EventLoopBox::new(event_loop));
            return;
        }

        // Occlusion pauses the window's animation frames; lifting it
        // resumes them with a redraw request (hidden-window behavior).
        if let WindowEvent::Occluded(occluded) = &event
            && let Some(raf) = {
                let state = self.lifecycle.state();
                state.windows.get(&window_id).map(|e| Rc::clone(&e.raf))
            }
        {
            let resume = !occluded && raf.has_pending();
            raf.set_paused(*occluded);
            if resume
                && let Some(view) = {
                    let state = self.lifecycle.state();
                    state.windows.get(&window_id).map(|e| Rc::clone(&e.view))
                }
            {
                view.borrow().request_redraw();
            }
        }

        // Redraw: run the window's queued animation frame callbacks
        // before the view renders, so their DOM changes land in this
        // frame. The callbacks re-enter JS, so the queue handle is cloned
        // out and no state borrow is held across the run.
        if matches!(event, WindowEvent::RedrawRequested)
            && let Some(raf) = {
                let state = self.lifecycle.state();
                state.windows.get(&window_id).map(|e| Rc::clone(&e.raf))
            }
        {
            let callbacks = raf.take_pending();
            raf.run(self.lifecycle.env(), callbacks);
        }

        // Forward non-close events to the View's event handler.
        //
        // `handle_winit_event` may re-enter JS (click -> spawn ->
        // openWindow). During that re-entry JS may call `NativeApp`
        // methods, which borrow the lifecycle state. So we must not
        // hold the outer state borrow across `handle_winit_event`.
        //
        // `view` is `Rc<RefCell<View>>`: we clone the Rc out while
        // holding a short state borrow, drop the state borrow, then call
        // into the view. The view's own RefCell borrow is held across
        // the JS callback, but re-entrant JS never touches *this* view
        // except through a fresh state borrow (which no longer
        // conflicts), so this is safe.
        let view_rc = {
            let state = self.lifecycle.state();
            state.windows.get(&window_id).map(|e| Rc::clone(&e.view))
        };
        if let Some(view_rc) = view_rc {
            view_rc.borrow_mut().handle_winit_event(event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.lifecycle
            .drain_opening_windows(&EventLoopBox::new(event_loop), Rc::clone(&self.lifecycle));
        self.lifecycle
            .drain_shell_events(&EventLoopBox::new(event_loop));
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let state = self.lifecycle.state();
        for entry in state.windows.values() {
            entry.view.borrow_mut().suspend();
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let state = self.lifecycle.state();
        for entry in state.windows.values() {
            entry.view.borrow_mut().suspend();
        }
    }
}
