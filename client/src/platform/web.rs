use std::sync::Arc;

use futures::channel::oneshot;
use wasm_bindgen::JsCast;
use winit::{
    event_loop::ActiveEventLoop,
    platform::web::WindowAttributesExtWebSys,
    window::{Window, WindowAttributes},
};

use crate::renderer::Renderer;

pub fn init_logging() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("Failed to initialize logger!");
}

pub fn window_attributes() -> WindowAttributes {
    Window::default_attributes().with_canvas(Some(canvas()))
}

/// On web, `window.inner_size()` may not yet reflect the canvas dimensions
/// at init time, so we read directly from the DOM element.
pub fn initial_size(_window: &Arc<Window>) -> (u32, u32) {
    let c = canvas();
    (c.width(), c.height())
}

pub fn spawn_renderer(
    window: Arc<Window>,
    width: u32,
    height: u32,
    sender: oneshot::Sender<Renderer>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let renderer = Renderer::new(window, width, height).await;
        if sender.send(renderer).is_err() {
            log::error!("Failed to create and send renderer!");
        }
    });
}

/// On web there is no application exit, but we do need to release the
/// pointer lock so the browser cursor is restored.
pub fn on_escape(_event_loop: &ActiveEventLoop) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.exit_pointer_lock();
        }
    }
}

/// Fetches the canvas element from the DOM. Panics if the element is
/// missing or has the wrong type, which is a fatal configuration error.
fn canvas() -> web_sys::HtmlCanvasElement {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap()
}
