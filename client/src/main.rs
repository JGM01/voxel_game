// #![windows_subsystem = "windows"] // uncomment this to suppress terminal on windows

fn main() -> Result<(), winit::error::EventLoopError> {
    let server_url =
        app_core::net::server_url_from_arg(std::env::args().nth(1)).unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });

    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = app_core::App::new(server_url);
    event_loop.run_app(&mut app)?;
    Ok(())
}
