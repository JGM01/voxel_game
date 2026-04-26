use std::sync::Arc;

use futures::channel::oneshot;
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

use crate::{
    input::{InputAccumulator, Interaction},
    net::{NetworkClient, NetworkEvent, TryRecvNetworkEventError},
    platform,
    renderer::Renderer,
    sim,
    world::World,
};

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_receiver: Option<oneshot::Receiver<Renderer>>,
    last_render_time: Option<Instant>,
    last_size: (u32, u32),
    world: World,
    input: InputAccumulator,
    cursor_locked: bool,
    network: Option<NetworkClient>,
    network_disconnected: bool,
    event_proxy: Option<EventLoopProxy<AppEvent>>,
}

#[derive(Clone, Copy, Debug)]
pub enum AppEvent {
    CursorLockChanged(bool),
}

impl App {
    pub fn new(server_url: String) -> Self {
        Self {
            network: Some(NetworkClient::connect(server_url)),
            ..Default::default()
        }
    }

    pub fn with_event_proxy(mut self, event_proxy: EventLoopProxy<AppEvent>) -> Self {
        self.event_proxy = Some(event_proxy);
        self
    }

    fn drain_network(&mut self, event_loop: &ActiveEventLoop) {
        if self.network_disconnected {
            return;
        }

        let Some(network) = self.network.as_ref() else {
            return;
        };

        loop {
            match network.try_recv() {
                Ok(NetworkEvent::Message(message)) => {
                    if matches!(message, shared::protocol::ServerMessage::Welcome { .. }) {
                        platform::on_network_connected();
                    }

                    if let Err(error) = self.world.apply_server_message(message) {
                        log::error!("Server error: {error}");
                        event_loop.exit();
                        return;
                    }
                }
                Ok(NetworkEvent::Fatal(error)) => {
                    log::error!("Network error: {error}");
                    self.network_disconnected = true;
                    self.network = None;
                    platform::on_network_disconnect(&error);
                    #[cfg(not(target_arch = "wasm32"))]
                    event_loop.exit();
                    return;
                }
                Err(TryRecvNetworkEventError::Empty) => return,
                Err(TryRecvNetworkEventError::Disconnected) => {
                    log::error!("Network thread disconnected");
                    self.network_disconnected = true;
                    self.network = None;
                    platform::on_network_disconnect("network thread disconnected");
                    #[cfg(not(target_arch = "wasm32"))]
                    event_loop.exit();
                    return;
                }
            }
        }
    }

    fn send_network_message(&self, message: shared::protocol::ClientMessage) {
        if self.network_disconnected {
            return;
        }

        if let Some(network) = self.network.as_ref() {
            network.send(message);
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::CursorLockChanged(locked) => {
                self.cursor_locked = locked;
            }
        }
    }

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
        if let Some(event_proxy) = self.event_proxy.as_ref() {
            platform::install_cursor_lock_observer(event_proxy.clone());
        }

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
                self.input.process_mouse(delta.0, delta.1);
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

        self.drain_network(event_loop);

        let Some(window) = self.window.as_ref() else {
            return;
        };

        match event {
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button,
                ..
            } => {
                if !self.cursor_locked {
                    self.cursor_locked = platform::lock_cursor(window);
                } else {
                    match button {
                        winit::event::MouseButton::Left => {
                            self.input.queue_interact(Interaction::Break);
                        }
                        winit::event::MouseButton::Right => {
                            self.input.queue_interact(Interaction::Place);
                        }
                        _ => {}
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
                            platform::unlock_cursor(window);
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
                self.input.process_key(key_code, state);
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width == 0 || height == 0 {
                    return;
                }
                log::info!("Resizing renderer surface to: ({width}, {height})");
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(width, height);
                }
                self.last_size = (width, height);
            }
            WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let messages = if let (Some(renderer), Some(last_render_time)) =
                    (self.renderer.as_mut(), self.last_render_time.as_mut())
                {
                    let now = Instant::now();
                    let delta_time = (now - *last_render_time).as_secs_f32();
                    *last_render_time = now;

                    self.world.player.camera.aspect = renderer.gpu.aspect_ratio();
                    let input = self.input.consume();
                    let messages = sim::tick(&mut self.world, &input, delta_time);
                    renderer.sync(&mut self.world);
                    renderer.render(&self.world);
                    messages
                } else {
                    Vec::new()
                };

                for message in messages {
                    self.send_network_message(message);
                }
            }
            _ => (),
        }

        window.request_redraw();
    }
}
