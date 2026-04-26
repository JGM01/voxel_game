use std::sync::Arc;

use std::future::Future;
use winit::{
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{CursorGrabMode, Window, WindowAttributes},
};

use crate::events::AppEvent;

pub fn init_logging() {
    env_logger::init();
}

pub fn window_attributes() -> WindowAttributes {
    Window::default_attributes().with_title("Standalone Winit/Wgpu Example")
}

pub fn initial_size(window: &Arc<Window>) -> (u32, u32) {
    let size = window.inner_size();
    (size.width, size.height)
}

pub fn spawn_local<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    pollster::block_on(future);
}

pub fn on_escape(_event_loop: &ActiveEventLoop) {
    // Native cursor release is handled through winit in the app event loop.
}

pub fn lock_cursor(window: &Window) -> bool {
    let _ = window.set_cursor_grab(CursorGrabMode::Locked);
    window.set_cursor_visible(false);
    true
}

pub fn unlock_cursor(window: &Window) {
    let _ = window.set_cursor_grab(CursorGrabMode::None);
    window.set_cursor_visible(true);
}

pub fn on_network_disconnect(_message: &str) {}

pub fn on_network_connected() {}

/// No-op on native — the OS drives `WindowEvent::Resized` directly.
pub fn install_canvas_resizer(_window: Arc<Window>) {}

pub fn install_cursor_lock_observer(_event_proxy: EventLoopProxy<AppEvent>) {}
