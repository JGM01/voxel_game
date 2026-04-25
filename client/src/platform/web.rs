use std::sync::Arc;

use futures::channel::oneshot;
use js_sys::Array;
use wasm_bindgen::{JsCast, closure::Closure};
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
/// at init time, so read directly from the DOM element instead.
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

/// Installs a `ResizeObserver` on the canvas so that CSS-driven resizes
/// (i.e. the user resizing their browser window) propagate into winit as
/// `WindowEvent::Resized` events.
///
/// Without this, the wgpu surface dimensions and the canvas CSS dimensions
/// can silently diverge, causing stretched or clipped rendering.
///
/// The `Closure` is intentionally `forget()`'d because it must remain alive
/// for the entire page lifetime. There is no earlier point at which
/// you would want to deregister the observer.
pub fn install_canvas_resizer(window: Arc<Window>) {
    let closure = Closure::wrap(Box::new(move |entries: Array| {
        if let Some(entry) = entries.get(0).dyn_ref::<web_sys::ResizeObserverEntry>() {
            let rect = entry.content_rect();
            let w = rect.width() as u32;
            let h = rect.height() as u32;
            if w > 0 && h > 0 {
                // `request_inner_size` signals winit to emit a
                // `WindowEvent::Resized`, which the existing handler already
                // deals with correctly.
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
            }
        }
    }) as Box<dyn FnMut(Array)>);

    let observer = web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref())
        .expect("Failed to construct ResizeObserver");

    observer.observe(&canvas());

    // Intentional: both the closure and the observer must outlive this
    // function. Leaking them is correct here.
    closure.forget();
    // Keep the observer alive too, dropping it would disconnect it.
    std::mem::forget(observer);
}

/// On web there is no application-level exit, but we must release the
/// pointer lock so the browser cursor is restored.
pub fn on_escape(_event_loop: &ActiveEventLoop) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.exit_pointer_lock();
        }
    }
}

/// Fetches the canvas element from the DOM. Panics if the element is
/// missing or is the wrong type; both are fatal configuration errors.
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
