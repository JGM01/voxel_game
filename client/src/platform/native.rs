use std::sync::Arc;

use futures::channel::oneshot;
use winit::{
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use crate::renderer::Renderer;

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

/// Blocks the calling thread until the renderer is constructed, then
/// immediately resolves the channel. From app.rs's perspective this is
/// identical to the async web path — the receiver just has a value ready
/// on the very first poll.
pub fn spawn_renderer(
    window: Arc<Window>,
    width: u32,
    height: u32,
    sender: oneshot::Sender<Renderer>,
) {
    let renderer = pollster::block_on(Renderer::new(window, width, height));
    let _ = sender.send(renderer);
}

pub fn on_escape(event_loop: &ActiveEventLoop) {
    event_loop.exit();
}
