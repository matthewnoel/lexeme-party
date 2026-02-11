use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use super::hud::{rasterize_multiline_text, rasterize_word_texture};

const CIRCLE_SEGMENTS: usize = 28;

// ---------------------------------------------------------------------------
// Vertex / instance types
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UnitVertex {
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
pub struct CircleInstance {
    pub pos: [f32; 2],
    pub radius: f32,
    pub color: [f32; 3],
    pub _pad: f32,
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl TextVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// RenderState
// ---------------------------------------------------------------------------

pub struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    unit_vertex_buffer: wgpu::Buffer,
    unit_vertex_count: u32,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    screen_uniform_buffer: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_bind_group: wgpu::BindGroup,
    text_sampler: wgpu::Sampler,
    text_texture: wgpu::Texture,
    text_view: wgpu::TextureView,
    text_size_px: [u32; 2],
    text_vertex_buffer: wgpu::Buffer,
    text_index_buffer: wgpu::Buffer,
    text_index_count: u32,
    cached_word: String,
    cached_style_hash: u64,
    leaderboard_bind_group: wgpu::BindGroup,
    leaderboard_texture: wgpu::Texture,
    leaderboard_view: wgpu::TextureView,
    leaderboard_size_px: [u32; 2],
    leaderboard_vertex_buffer: wgpu::Buffer,
    cached_leaderboard_hash: u64,
}

impl RenderState {
    pub async fn new(window: &'static winit::window::Window) -> anyhow::Result<Self> {
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
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/circle.wgsl").into()),
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

        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("text-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let (text_texture, text_view) = create_text_texture(&device, 1, 1, "text-texture-initial");
        let text_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text-bind-group"),
            layout: &text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&text_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&text_sampler),
                },
            ],
        });

        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
        });
        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text-pipeline-layout"),
            bind_group_layouts: &[&screen_bind_group_layout, &text_bind_group_layout],
            push_constant_ranges: &[],
        });
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text-pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: "vs_main",
                buffers: &[TextVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
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

        let text_vertex_init = [
            TextVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            TextVertex {
                pos: [0.0, 0.0],
                uv: [1.0, 0.0],
            },
            TextVertex {
                pos: [0.0, 0.0],
                uv: [1.0, 1.0],
            },
            TextVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 1.0],
            },
        ];
        let text_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text-vertex-buffer"),
            contents: bytemuck::cast_slice(&text_vertex_init),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let text_index_data: [u16; 6] = [0, 1, 2, 2, 3, 0];
        let text_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text-index-buffer"),
            contents: bytemuck::cast_slice(&text_index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (leaderboard_texture, leaderboard_view) =
            create_text_texture(&device, 1, 1, "leaderboard-texture-initial");
        let leaderboard_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("leaderboard-bind-group"),
            layout: &text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&leaderboard_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&text_sampler),
                },
            ],
        });
        let leaderboard_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("leaderboard-vertex-buffer"),
                contents: bytemuck::cast_slice(&text_vertex_init),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
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
            text_pipeline,
            text_bind_group_layout,
            text_bind_group,
            text_sampler,
            text_texture,
            text_view,
            text_size_px: [1, 1],
            text_vertex_buffer,
            text_index_buffer,
            text_index_count: text_index_data.len() as u32,
            cached_word: String::new(),
            cached_style_hash: 0,
            leaderboard_bind_group,
            leaderboard_texture,
            leaderboard_view,
            leaderboard_size_px: [1, 1],
            leaderboard_vertex_buffer,
            cached_leaderboard_hash: 0,
        })
    }

    pub fn screen_size(&self) -> [f32; 2] {
        [self.size.width as f32, self.size.height as f32]
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
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
        self.update_text_quad_vertices();
        self.update_leaderboard_quad_vertices();
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

    fn update_word_texture(&mut self, word: &str, letter_colors: &[[u8; 4]]) {
        let style_hash = letter_colors_hash(letter_colors);
        if word == self.cached_word && style_hash == self.cached_style_hash {
            return;
        }
        self.cached_word = word.to_string();
        self.cached_style_hash = style_hash;

        let (pixels, width, height) = rasterize_word_texture(word, letter_colors);
        if width == 0 || height == 0 {
            return;
        }

        if self.text_size_px != [width, height] {
            let (texture, view) = create_text_texture(&self.device, width, height, "text-texture");
            self.text_texture = texture;
            self.text_view = view;
            self.text_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("text-bind-group"),
                layout: &self.text_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.text_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                    },
                ],
            });
            self.text_size_px = [width, height];
        }

        write_texture_padded(&self.queue, &self.text_texture, &pixels, width, height);
        self.update_text_quad_vertices();
    }

    fn update_text_quad_vertices(&mut self) {
        let w = self.text_size_px[0] as f32;
        let h = self.text_size_px[1] as f32;
        let screen_w = self.size.width as f32;
        let x = ((screen_w - w) * 0.5).max(8.0);
        let y = 20.0;
        let vertices = quad_vertices(x, y, w, h);
        self.queue
            .write_buffer(&self.text_vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    fn update_leaderboard_quad_vertices(&mut self) {
        let w = self.leaderboard_size_px[0] as f32;
        let h = self.leaderboard_size_px[1] as f32;
        let x = 20.0;
        let y = 80.0;
        let vertices = quad_vertices(x, y, w, h);
        self.queue.write_buffer(
            &self.leaderboard_vertex_buffer,
            0,
            bytemuck::cast_slice(&vertices),
        );
    }

    fn update_leaderboard_texture(&mut self, leaderboard_lines: &[(String, [u8; 4])]) {
        let hash = leaderboard_lines_hash(leaderboard_lines);
        if hash == self.cached_leaderboard_hash {
            return;
        }
        self.cached_leaderboard_hash = hash;

        let (pixels, width, height) = rasterize_multiline_text(leaderboard_lines, 3, 2, 4);
        if width == 0 || height == 0 {
            return;
        }

        if self.leaderboard_size_px != [width, height] {
            let (texture, view) =
                create_text_texture(&self.device, width, height, "leaderboard-texture");
            self.leaderboard_texture = texture;
            self.leaderboard_view = view;
            self.leaderboard_bind_group =
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("leaderboard-bind-group"),
                    layout: &self.text_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.leaderboard_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                        },
                    ],
                });
            self.leaderboard_size_px = [width, height];
        }

        write_texture_padded(
            &self.queue,
            &self.leaderboard_texture,
            &pixels,
            width,
            height,
        );
        self.update_leaderboard_quad_vertices();
    }

    pub fn render(
        &mut self,
        instances: &[CircleInstance],
        current_word: &str,
        letter_colors: &[[u8; 4]],
        leaderboard_lines: &[(String, [u8; 4])],
    ) -> Result<(), wgpu::SurfaceError> {
        self.update_word_texture(current_word, letter_colors);
        self.update_leaderboard_texture(leaderboard_lines);
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

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.text_pipeline);
            pass.set_bind_group(0, &self.screen_bind_group, &[]);
            pass.set_bind_group(1, &self.text_bind_group, &[]);
            pass.set_vertex_buffer(0, self.text_vertex_buffer.slice(..));
            pass.set_index_buffer(self.text_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.text_index_count, 0, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("leaderboard-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.text_pipeline);
            pass.set_bind_group(0, &self.screen_bind_group, &[]);
            pass.set_bind_group(1, &self.leaderboard_bind_group, &[]);
            pass.set_vertex_buffer(0, self.leaderboard_vertex_buffer.slice(..));
            pass.set_index_buffer(self.text_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.text_index_count, 0, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

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

fn create_text_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn write_texture_padded(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    pixels: &[u8],
    width: u32,
    height: u32,
) {
    let bytes_per_row_unpadded = width * 4;
    let bytes_per_row_padded = bytes_per_row_unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let mut padded = vec![0u8; (bytes_per_row_padded * height) as usize];
    for row in 0..height as usize {
        let src_start = row * bytes_per_row_unpadded as usize;
        let src_end = src_start + bytes_per_row_unpadded as usize;
        let dst_start = row * bytes_per_row_padded as usize;
        let dst_end = dst_start + bytes_per_row_unpadded as usize;
        padded[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
    }

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &padded,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row_padded),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn letter_colors_hash(colors: &[[u8; 4]]) -> u64 {
    let mut h = 1469598103934665603u64;
    for c in colors {
        for b in c {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211u64);
        }
    }
    h
}

fn leaderboard_lines_hash(lines: &[(String, [u8; 4])]) -> u64 {
    let mut h = 1469598103934665603u64;
    for (line, color) in lines {
        for b in line.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211u64);
        }
        for b in color {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211u64);
        }
    }
    h
}

fn quad_vertices(x: f32, y: f32, w: f32, h: f32) -> [TextVertex; 4] {
    [
        TextVertex {
            pos: [x, y],
            uv: [0.0, 0.0],
        },
        TextVertex {
            pos: [x + w, y],
            uv: [1.0, 0.0],
        },
        TextVertex {
            pos: [x + w, y + h],
            uv: [1.0, 1.0],
        },
        TextVertex {
            pos: [x, y + h],
            uv: [0.0, 1.0],
        },
    ]
}
