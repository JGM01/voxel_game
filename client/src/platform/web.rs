use std::sync::Arc;

use std::future::Future;
use wasm_bindgen::{JsCast, closure::Closure};
use winit::{
    event_loop::{ActiveEventLoop, EventLoopProxy},
    platform::web::WindowAttributesExtWebSys,
    window::{Window, WindowAttributes},
};

use crate::events::AppEvent;

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
    let (width, height) = viewport_size();
    apply_canvas_size(&canvas(), width, height);
    (width, height)
}

pub fn spawn_local<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
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
    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let (width, height) = viewport_size();
        apply_canvas_size(&canvas(), width, height);
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(width, height));
    }) as Box<dyn FnMut(_)>);

    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .expect("failed to install window resize listener");
    closure.forget();
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

pub fn lock_cursor(window: &Window) -> bool {
    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
    window.set_cursor_visible(false);
    true
}

pub fn unlock_cursor(window: &Window) {
    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
    window.set_cursor_visible(true);
}

pub fn install_cursor_lock_observer(event_proxy: EventLoopProxy<AppEvent>) {
    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let _ = event_proxy.send_event(AppEvent::CursorLockChanged(is_canvas_pointer_locked()));
    }) as Box<dyn FnMut(_)>);

    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .add_event_listener_with_callback("pointerlockchange", closure.as_ref().unchecked_ref())
        .expect("failed to install pointer lock observer");
    closure.forget();
}

pub fn on_network_disconnect(message: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(status) = document.get_element_by_id("connect-status") {
                status.set_text_content(Some(&format!(
                    "Disconnected: {message}. Reload the page to connect again."
                )));
            }
        }
    }
}

pub fn on_network_connected() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(status) = document.get_element_by_id("connect-status") {
                status.set_text_content(Some("Connected."));
            }
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

fn viewport_size() -> (u32, u32) {
    let window = web_sys::window().unwrap();
    let dpr = window.device_pixel_ratio();
    let canvas = canvas();
    let width = (canvas.client_width() as f64 * dpr).max(1.0) as u32;
    let height = (canvas.client_height() as f64 * dpr).max(1.0) as u32;
    (width, height)
}

fn apply_canvas_size(canvas: &web_sys::HtmlCanvasElement, width: u32, height: u32) {
    canvas.set_width(width);
    canvas.set_height(height);
}

fn is_canvas_pointer_locked() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(pointer_lock_element) = document.pointer_lock_element() else {
        return false;
    };
    let canvas: web_sys::Element = canvas().into();
    pointer_lock_element == canvas
}
