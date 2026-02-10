use crate::protocol::{ClientMessage, PlayerState, ServerMessage};
use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{
    collections::HashMap,
    sync::mpsc as std_mpsc,
    thread,
    time::Instant,
};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use wgpu::util::DeviceExt;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

const BASE_RADIUS: f32 = 16.0;
const SCORE_RADIUS_STEP: f32 = 4.0;
const GRAVITY_TO_CENTER: f32 = 42.0;
const VELOCITY_DAMPING: f32 = 0.90;
const CIRCLE_SEGMENTS: usize = 28;

#[derive(Debug)]
enum NetworkEvent {
    Server(ServerMessage),
    Disconnected(String),
}

#[derive(Debug, Clone)]
struct RenderPlayer {
    id: u64,
    score: u32,
    pos: [f32; 2],
    vel: [f32; 2],
    color: [f32; 3],
}

impl RenderPlayer {
    fn radius(&self) -> f32 {
        BASE_RADIUS + self.score as f32 * SCORE_RADIUS_STEP
    }
}

struct GameClient {
    local_name: String,
    local_player_id: Option<u64>,
    round: u32,
    current_word: String,
    typed_word: String,
    winner_last_round: Option<String>,
    players: HashMap<u64, RenderPlayer>,
    net_tx: mpsc::UnboundedSender<ClientMessage>,
}

impl GameClient {
    fn new(local_name: String, net_tx: mpsc::UnboundedSender<ClientMessage>) -> Self {
        Self {
            local_name,
            local_player_id: None,
            round: 1,
            current_word: "waiting".to_string(),
            typed_word: String::new(),
            winner_last_round: None,
            players: HashMap::new(),
            net_tx,
        }
    }

    fn apply_server_msg(&mut self, msg: ServerMessage, screen_size: [f32; 2]) {
        match msg {
            ServerMessage::Welcome { player_id } => {
                self.local_player_id = Some(player_id);
            }
            ServerMessage::State {
                round,
                current_word,
                players,
                winner_last_round,
            } => {
                self.round = round;
                self.current_word = current_word;
                self.winner_last_round = winner_last_round;
                self.sync_players(players, screen_size);
            }
        }
    }

    fn sync_players(&mut self, incoming: Vec<PlayerState>, screen_size: [f32; 2]) {
        let mut rng = rand::thread_rng();
        let half_w = (screen_size[0] * 0.5).max(1.0);
        let half_h = (screen_size[1] * 0.5).max(1.0);

        let mut next_map = HashMap::new();
        for p in incoming {
            if let Some(existing) = self.players.remove(&p.id) {
                next_map.insert(
                    p.id,
                    RenderPlayer {
                        id: p.id,
                        score: p.score,
                        ..existing
                    },
                );
            } else {
                let x = rng.gen_range(-half_w * 0.6..half_w * 0.6);
                let y = rng.gen_range(-half_h * 0.6..half_h * 0.6);
                next_map.insert(
                    p.id,
                    RenderPlayer {
                        id: p.id,
                        score: p.score,
                        pos: [x, y],
                        vel: [0.0, 0.0],
                        color: color_from_id(p.id),
                    },
                );
            }
        }

        self.players = next_map;
    }

    fn step_physics(&mut self, dt: f32, screen_size: [f32; 2]) {
        if self.players.is_empty() {
            return;
        }

        let ids: Vec<u64> = self.players.keys().copied().collect();
        for id in &ids {
            if let Some(p) = self.players.get_mut(id) {
                let fx = -p.pos[0] * GRAVITY_TO_CENTER;
                let fy = -p.pos[1] * GRAVITY_TO_CENTER;
                p.vel[0] += fx * dt;
                p.vel[1] += fy * dt;
                p.vel[0] *= VELOCITY_DAMPING;
                p.vel[1] *= VELOCITY_DAMPING;
                p.pos[0] += p.vel[0] * dt;
                p.pos[1] += p.vel[1] * dt;
            }
        }

        let mut pairs = Vec::new();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                pairs.push((ids[i], ids[j]));
            }
        }

        for (a_id, b_id) in pairs {
            let (a_pos, b_pos, a_r, b_r) =
                if let (Some(a), Some(b)) = (self.players.get(&a_id), self.players.get(&b_id)) {
                    (a.pos, b.pos, a.radius(), b.radius())
                } else {
                    continue;
                };

            let dx = b_pos[0] - a_pos[0];
            let dy = b_pos[1] - a_pos[1];
            let dist_sq = dx * dx + dy * dy;
            let min_dist = a_r + b_r + 2.0;
            if dist_sq <= f32::EPSILON {
                continue;
            }
            let dist = dist_sq.sqrt();
            if dist >= min_dist {
                continue;
            }

            let nx = dx / dist;
            let ny = dy / dist;
            let push = (min_dist - dist) * 0.5;

            if let Some(a) = self.players.get_mut(&a_id) {
                a.pos[0] -= nx * push;
                a.pos[1] -= ny * push;
            }
            if let Some(b) = self.players.get_mut(&b_id) {
                b.pos[0] += nx * push;
                b.pos[1] += ny * push;
            }
        }

        let limit_x = (screen_size[0] * 0.5).max(1.0);
        let limit_y = (screen_size[1] * 0.5).max(1.0);
        for p in self.players.values_mut() {
            let r = p.radius();
            p.pos[0] = p.pos[0].clamp(-limit_x + r, limit_x - r);
            p.pos[1] = p.pos[1].clamp(-limit_y + r, limit_y - r);
        }
    }

    fn handle_key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::Backspace) => {
                self.typed_word.pop();
            }
            Key::Named(NamedKey::Enter) => {
                self.try_submit();
            }
            Key::Character(s) => {
                for c in s.chars() {
                    if c.is_ascii_alphabetic() {
                        self.typed_word.push(c.to_ascii_lowercase());
                    }
                }
                if self.typed_word.eq_ignore_ascii_case(&self.current_word) {
                    self.try_submit();
                }
            }
            _ => {}
        }
    }

    fn try_submit(&mut self) {
        if self.typed_word.eq_ignore_ascii_case(&self.current_word) && !self.current_word.is_empty() {
            let _ = self.net_tx.send(ClientMessage::SubmitWord {
                word: self.typed_word.clone(),
            });
            self.typed_word.clear();
        }
    }

    fn build_instances(&self) -> Vec<CircleInstance> {
        let mut list = Vec::with_capacity(self.players.len());
        for player in self.players.values() {
            let mut color = player.color;
            if Some(player.id) == self.local_player_id {
                color = [1.0, 0.95, 0.35];
            }
            list.push(CircleInstance {
                pos: player.pos,
                radius: player.radius(),
                color,
                _pad: 0.0,
            });
        }
        list
    }

    fn update_window_title(&self, window: &winit::window::Window) {
        let my_score = self
            .local_player_id
            .and_then(|id| self.players.get(&id).map(|p| p.score))
            .unwrap_or(0);
        let winner = self
            .winner_last_round
            .as_ref()
            .map_or("none".to_string(), |w| w.clone());
        let title = format!(
            "Lexeme Party | Round {} | Word: {} | Typed: {} | You: {} ({}) | Last winner: {}",
            self.round,
            self.current_word,
            self.typed_word,
            self.local_name,
            my_score,
            winner
        );
        window.set_title(&title);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UnitVertex {
    pos: [f32; 2],
}

impl UnitVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UnitVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CircleInstance {
    pos: [f32; 2],
    radius: f32,
    color: [f32; 3],
    _pad: f32,
}

impl CircleInstance {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CircleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenUniform {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    unit_vertex_buffer: wgpu::Buffer,
    unit_vertex_count: u32,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    screen_uniform_buffer: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
}

impl RenderState {
    async fn new(window: &'static winit::window::Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find a suitable GPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("lexeme-party-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let unit_vertices = build_circle_unit_vertices(CIRCLE_SEGMENTS);
        let unit_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("circle-unit-vertices"),
            contents: bytemuck::cast_slice(&unit_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let screen_uniform = ScreenUniform {
            screen_size: [config.width as f32, config.height as f32],
            _pad: [0.0, 0.0],
        };
        let screen_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("screen-uniform"),
            contents: bytemuck::bytes_of(&screen_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("screen-bind-group-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen-bind-group"),
            layout: &screen_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("circle-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("circle.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-pipeline-layout"),
            bind_group_layouts: &[&screen_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[UnitVertex::desc(), CircleInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let initial_capacity = 64usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance-buffer"),
            size: (initial_capacity * std::mem::size_of::<CircleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            unit_vertex_buffer,
            unit_vertex_count: unit_vertices.len() as u32,
            instance_buffer,
            instance_capacity: initial_capacity,
            screen_uniform_buffer,
            screen_bind_group,
        })
    }

    fn screen_size(&self) -> [f32; 2] {
        [self.size.width as f32, self.size.height as f32]
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        let uniform = ScreenUniform {
            screen_size: [new_size.width as f32, new_size.height as f32],
            _pad: [0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.screen_uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    fn ensure_instance_capacity(&mut self, count: usize) {
        if count <= self.instance_capacity {
            return;
        }
        self.instance_capacity = count.next_power_of_two();
        self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance-buffer"),
            size: (self.instance_capacity * std::mem::size_of::<CircleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn render(&mut self, instances: &[CircleInstance]) -> Result<(), wgpu::SurfaceError> {
        self.ensure_instance_capacity(instances.len());
        if !instances.is_empty() {
            self.queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.06,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.screen_bind_group, &[]);
            pass.set_vertex_buffer(0, self.unit_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            pass.draw(0..self.unit_vertex_count, 0..instances.len() as u32);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn build_circle_unit_vertices(segments: usize) -> Vec<UnitVertex> {
    let mut vertices = Vec::with_capacity(segments * 3);
    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        vertices.push(UnitVertex { pos: [0.0, 0.0] });
        vertices.push(UnitVertex {
            pos: [a0.cos(), a0.sin()],
        });
        vertices.push(UnitVertex {
            pos: [a1.cos(), a1.sin()],
        });
    }
    vertices
}

fn color_from_id(id: u64) -> [f32; 3] {
    let mut x = id.wrapping_mul(0x9E37_79B1_85EB_CA87);
    x ^= x >> 33;
    let r = ((x & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    let g = (((x >> 8) & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    let b = (((x >> 16) & 0xFF) as f32 / 255.0) * 0.6 + 0.25;
    [r.min(1.0), g.min(1.0), b.min(1.0)]
}

fn spawn_network(
    ws_url: String,
    name: String,
) -> (
    mpsc::UnboundedSender<ClientMessage>,
    std_mpsc::Receiver<NetworkEvent>,
) {
    let (to_net_tx, to_net_rx) = mpsc::unbounded_channel::<ClientMessage>();
    let (to_ui_tx, to_ui_rx) = std_mpsc::channel::<NetworkEvent>();

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = to_ui_tx.send(NetworkEvent::Disconnected(e.to_string()));
                return;
            }
        };

        let result = runtime.block_on(network_task(ws_url, name, to_net_rx, to_ui_tx.clone()));
        if let Err(err) = result {
            let _ = to_ui_tx.send(NetworkEvent::Disconnected(err.to_string()));
        }
    });

    (to_net_tx, to_ui_rx)
}

async fn network_task(
    ws_url: String,
    name: String,
    mut outbound_rx: mpsc::UnboundedReceiver<ClientMessage>,
    inbound_tx: std_mpsc::Sender<NetworkEvent>,
) -> anyhow::Result<()> {
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .with_context(|| format!("failed connecting to {}", ws_url))?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let join = serde_json::to_string(&ClientMessage::Join { name })?;
    ws_write.send(Message::Text(join)).await?;

    loop {
        tokio::select! {
            Some(outbound) = outbound_rx.recv() => {
                let payload = serde_json::to_string(&outbound)?;
                ws_write.send(Message::Text(payload)).await?;
            }
            incoming = ws_read.next() => {
                let Some(msg_result) = incoming else { break; };
                let msg = msg_result?;
                if !msg.is_text() {
                    continue;
                }
                let text = msg.into_text()?;
                let server_msg: ServerMessage = serde_json::from_str(&text)?;
                let _ = inbound_tx.send(NetworkEvent::Server(server_msg));
            }
        }
    }

    Ok(())
}

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
                    game.update_window_title(&window);

                    let instances = game.build_instances();
                    match render.render(&instances) {
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
