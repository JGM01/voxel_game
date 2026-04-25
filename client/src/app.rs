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

#[cfg(not(target_arch = "wasm32"))]
use crate::net::{NetworkClient, NetworkEvent};

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_receiver: Option<oneshot::Receiver<Renderer>>,
    last_render_time: Option<Instant>,
    last_size: (u32, u32),
    cursor_locked: bool,
    #[cfg(not(target_arch = "wasm32"))]
    network: Option<NetworkClient>,
}

#[cfg(not(target_arch = "wasm32"))]
impl App {
    pub fn new(server_url: String) -> Self {
        Self {
            network: Some(NetworkClient::connect(server_url)),
            ..Default::default()
        }
    }

    fn drain_network(&mut self, event_loop: &ActiveEventLoop) {
        let Some(network) = self.network.as_ref() else {
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        loop {
            match network.try_recv() {
                Ok(NetworkEvent::Message(message)) => {
                    if let Err(error) = renderer
                        .scene
                        .apply_server_message(&renderer.gpu.device, message)
                    {
                        log::error!("Server error: {error}");
                        event_loop.exit();
                        return;
                    }
                }
                Ok(NetworkEvent::Fatal(error)) => {
                    log::error!("Network error: {error}");
                    event_loop.exit();
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    log::error!("Network thread disconnected");
                    event_loop.exit();
                    return;
                }
            }
        }
    }

    fn send_network_message(&self, message: shared::protocol::ClientMessage) {
        if let Some(network) = self.network.as_ref() {
            network.send(message);
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl App {
    pub fn new(_server_url: String) -> Self {
        Self::default()
    }

    fn send_network_message(&self, _message: shared::protocol::ClientMessage) {}
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
                        .player_controller
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

        #[cfg(not(target_arch = "wasm32"))]
        self.drain_network(event_loop);

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
                    if let Some(message) = renderer
                        .scene
                        .interact(&renderer.gpu.device, is_right_click)
                    {
                        self.send_network_message(message);
                    }
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
                if state == winit::event::ElementState::Pressed {
                    match key_code {
                        winit::keyboard::KeyCode::Escape => {
                            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                            window.set_cursor_visible(true);
                            self.cursor_locked = false;
                            platform::on_escape(event_loop);
                        }
                        winit::keyboard::KeyCode::KeyQ => {
                            log::info!("Quit requested. Exiting...");
                            event_loop.exit();
                            return;
                        }
                        _ => {}
                    }
                }
                renderer
                    .scene
                    .player_controller
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
                if let Some(message) = renderer.render_frame(delta_time) {
                    self.send_network_message(message);
                }
            }
            _ => (),
        }

        window.request_redraw();
    }
}
