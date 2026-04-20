use std::sync::Arc;

use futures::channel::oneshot;
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{platform, renderer::Renderer};

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_receiver: Option<oneshot::Receiver<Renderer>>,
    last_render_time: Option<Instant>,
    last_size: (u32, u32),
    cursor_locked: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Ok(window) = event_loop.create_window(platform::window_attributes()) else {
            return;
        };

        let first_window_handle = self.window.is_none();
        let window_handle = Arc::new(window);
        self.window = Some(window_handle.clone());

        if !first_window_handle {
            return;
        }

        platform::init_logging();

        let (width, height) = platform::initial_size(&window_handle);
        self.last_size = (width, height);

        // Wire up CSS resize propagation on web. On native this is a no-op
        // because the OS drives WindowEvent::Resized directly.
        platform::install_canvas_resizer(window_handle.clone());

        let (sender, receiver) = oneshot::channel();
        self.renderer_receiver = Some(receiver);
        platform::spawn_renderer(window_handle, width, height, sender);

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
        if let Some(receiver) = self.renderer_receiver.as_mut() {
            if let Ok(Some(mut renderer)) = receiver.try_recv() {
                if let Some(window) = self.window.as_ref() {
                    let size = window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        renderer.resize(size.width, size.height);
                        self.last_size = (size.width, size.height);
                    }
                }
                self.renderer = Some(renderer);
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
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
                    window.set_cursor_visible(false);
                    self.cursor_locked = true;
                } else {
                    let is_right_click = button == winit::event::MouseButton::Right;
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
                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                    window.set_cursor_visible(true);
                    self.cursor_locked = false;
                    platform::on_escape(event_loop);
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
