mod game;
mod hud;
mod net;
pub mod render;

use std::time::Instant;

use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use game::GameClient;
use net::{spawn_network, NetworkEvent};
use render::RenderState;

pub fn run_client(ws_url: String, player_name: String) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window: &'static winit::window::Window = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title("Lexeme Party")
            .with_inner_size(PhysicalSize::new(1100, 720))
            .build(&event_loop)?,
    ));
    let mut render = pollster::block_on(RenderState::new(window))?;
    let (net_tx, net_rx) = spawn_network(ws_url, player_name.clone());
    let mut game = GameClient::new(player_name, net_tx);
    let mut last_tick = Instant::now();

    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    target.exit();
                }
                WindowEvent::Resized(size) => {
                    render.resize(size);
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed {
                        game.handle_key(&event.logical_key);
                    }
                }
                WindowEvent::RedrawRequested => {
                    while let Ok(net_event) = net_rx.try_recv() {
                        match net_event {
                            NetworkEvent::Server(msg) => {
                                let screen_size = render.screen_size();
                                game.apply_server_msg(msg, screen_size);
                            }
                            NetworkEvent::Disconnected(reason) => {
                                window.set_title(&format!("Disconnected: {}", reason));
                            }
                        }
                    }

                    let now = Instant::now();
                    let dt = (now - last_tick).as_secs_f32().min(0.05);
                    last_tick = now;

                    game.step_physics(dt, render.screen_size());
                    game.update_window_title(window);

                    let instances = game.build_instances();
                    let letter_colors = game.build_letter_colors();
                    let leaderboard_lines = game.build_leaderboard_lines();
                    match render.render(
                        &instances,
                        &game.current_word,
                        &letter_colors,
                        &leaderboard_lines,
                    ) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            render.resize(render.size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            target.exit();
                        }
                        Err(wgpu::SurfaceError::Timeout) => {}
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}
