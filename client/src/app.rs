#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use std::sync::Arc;
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::renderer::Renderer;

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    last_render_time: Option<Instant>,
    #[cfg(target_arch = "wasm32")]
    renderer_receiver: Option<futures::channel::oneshot::Receiver<Renderer>>,
    last_size: (u32, u32),
    cursor_locked: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();

        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_title("Standalone Winit/Wgpu Example");
        }

        #[allow(unused_assignments)]
        #[cfg(target_arch = "wasm32")]
        let (mut canvas_width, mut canvas_height) = (0, 0);

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = wgpu::web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<wgpu::web_sys::HtmlCanvasElement>()
                .unwrap();

            canvas_width = canvas.width();
            canvas_height = canvas.height();
            self.last_size = (canvas_width, canvas_height);
            attributes = attributes.with_canvas(Some(canvas));
        }

        let Ok(window) = event_loop.create_window(attributes) else {
            return;
        };

        let first_window_handle = self.window.is_none();
        let window_handle = Arc::new(window);
        self.window = Some(window_handle.clone());
        if !first_window_handle {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let inner_size = window_handle.inner_size();
            self.last_size = (inner_size.width, inner_size.height);
        }

        #[cfg(not(target_arch = "wasm32"))]
        let (width, height) = (
            window_handle.inner_size().width,
            window_handle.inner_size().height,
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            env_logger::init();
            let renderer = pollster::block_on(Renderer::new(window_handle.clone(), width, height));
            self.renderer = Some(renderer);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let (sender, receiver) = futures::channel::oneshot::channel();
            self.renderer_receiver = Some(receiver);
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init().expect("Failed to initialize logger!");
            log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
            wasm_bindgen_futures::spawn_local(async move {
                let renderer =
                    Renderer::new(window_handle.clone(), canvas_width, canvas_height).await;
                if sender.send(renderer).is_err() {
                    log::error!("Failed to create and send renderer!");
                }
            });
        }

        self.last_render_time = Some(Instant::now());
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            if self.cursor_locked {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer
                        .scene
                        .camera_controller
                        .process_mouse(delta.0, delta.1);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut renderer_received = false;
            if let Some(receiver) = self.renderer_receiver.as_mut() {
                if let Ok(Some(mut renderer)) = receiver.try_recv() {
                    // Sync initial size
                    if let Some(window) = self.window.as_ref() {
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            renderer.resize(size.width, size.height);
                            self.last_size = (size.width, size.height);
                        }
                    }

                    self.renderer = Some(renderer);
                    renderer_received = true;
                }
            }
            if renderer_received {
                self.renderer_receiver = None;
            }
        }

        let Some(window) = self.window.as_ref() else {
            return;
        };

        let (Some(renderer), Some(last_render_time)) =
            (self.renderer.as_mut(), self.last_render_time.as_mut())
        else {
            return;
        };

        match event {
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button,
                ..
            } => {
                if !self.cursor_locked {
                    // Lock cursor on first click
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
                    window.set_cursor_visible(false);
                    self.cursor_locked = true;
                } else {
                    // If already locked, perform world interaction
                    let is_right_click = button == winit::event::MouseButton::Right;

                    // Call the interact method, passing the wgpu device and click type
                    renderer
                        .scene
                        .interact(&renderer.gpu.device, is_right_click);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        state,
                        ..
                    },
                ..
            } => {
                if state == winit::event::ElementState::Pressed
                    && matches!(key_code, winit::keyboard::KeyCode::Escape)
                {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        event_loop.exit();
                    }
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                    window.set_cursor_visible(true);
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(web_window) = web_sys::window() {
                            if let Some(document) = web_window.document() {
                                document.exit_pointer_lock();
                            }
                        }
                    }
                    self.cursor_locked = false;
                }
                renderer
                    .scene
                    .camera_controller
                    .process_keyboard(key_code, state);
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width == 0 || height == 0 {
                    return;
                }

                log::info!("Resizing renderer surface to: ({width}, {height})");
                renderer.resize(width, height);
                self.last_size = (width, height);
            }
            WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now - *last_render_time;
                *last_render_time = now;

                renderer.render_frame(delta_time);
            }
            _ => (),
        }

        window.request_redraw();
    }
}
