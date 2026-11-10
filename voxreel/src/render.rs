//! Offscreen wgpu renderer: instanced cubes with per-corner AO, a shadow map from the sun,
//! a gridded ground plane and a gradient sky. Produces tightly packed sRGB RGBA8 frames.

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3};
use std::sync::LazyLock;
use wgpu::util::DeviceExt;

const SAMPLES: u32 = 4;
const SHADOW_SIZE: u32 = 4096;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    /// Min corner of the box in world space.
    pub pos: [f32; 3],
    /// sRGB colour, RGBA8.
    pub color: u32,
    pub size: [f32; 3],
    pub glow: f32,
    pub ao: [u32; 2],
}

impl Instance {
    /// A unit voxel at `pos` (min corner) shrunk around its centre by `scale`.
    pub fn cube(pos: Vec3, scale: f32, c: [u8; 3], glow: f32, ao: [u32; 2]) -> Self {
        Self {
            pos: (pos + Vec3::splat(0.5 * (1.0 - scale))).to_array(),
            color: u32::from_le_bytes([c[0], c[1], c[2], 255]),
            size: [scale; 3],
            glow,
            ao,
        }
    }

    /// An arbitrary box without AO (plates, pedestals).
    pub fn slab(min: Vec3, max: Vec3, c: [u8; 3]) -> Self {
        Self {
            pos: min.to_array(),
            color: u32::from_le_bytes([c[0], c[1], c[2], 255]),
            size: (max - min).to_array(),
            glow: 0.0,
            ao: [u32::MAX, u32::MAX >> 16],
        }
    }
}

pub struct Face {
    pub normal: IVec3,
    pub t1: IVec3,
    pub t2: IVec3,
    /// Corners in local [0,1]^3, counter-clockwise seen from outside.
    pub corners: [Vec3; 4],
}

/// Face/corner order shared by the vertex buffer and the AO packing in `anim`.
pub static FACES: LazyLock<[Face; 6]> = LazyLock::new(|| {
    let mk = |n: IVec3, t1: IVec3, t2: IVec3| {
        let c = Vec3::splat(0.5) + n.as_vec3() * 0.5;
        let (a, b) = (t1.as_vec3() * 0.5, t2.as_vec3() * 0.5);
        Face { normal: n, t1, t2, corners: [c - a - b, c + a - b, c + a + b, c - a + b] }
    };
    [
        mk(IVec3::X, IVec3::Y, IVec3::Z),
        mk(IVec3::NEG_X, IVec3::Z, IVec3::Y),
        mk(IVec3::Y, IVec3::Z, IVec3::X),
        mk(IVec3::NEG_Y, IVec3::X, IVec3::Z),
        mk(IVec3::Z, IVec3::X, IVec3::Y),
        mk(IVec3::NEG_Z, IVec3::Y, IVec3::X),
    ]
});

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    corner: u32,
}

fn cube_vertices() -> Vec<Vertex> {
    let mut v = Vec::with_capacity(36);
    for (f, face) in FACES.iter().enumerate() {
        for ci in [0usize, 1, 2, 0, 2, 3] {
            v.push(Vertex {
                pos: face.corners[ci].to_array(),
                normal: face.normal.as_vec3().to_array(),
                corner: (f * 4 + ci) as u32,
            });
        }
    }
    v
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    cam: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    ground: [f32; 4],
    /// fog_near, fog_far, shadow texel size, ground y
    params: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub sky_top: [u8; 3],
    pub sky_horizon: [u8; 3],
    pub ground: [u8; 3],
    pub plate: [u8; 3],
}

impl Theme {
    pub fn by_name(name: &str) -> Self {
        match name {
            "night" => Self { sky_top: [12, 14, 40], sky_horizon: [70, 52, 110], ground: [38, 36, 58], plate: [24, 22, 36] },
            "sunset" => Self { sky_top: [72, 92, 170], sky_horizon: [255, 170, 120], ground: [210, 170, 150], plate: [70, 50, 60] },
            "mint" => Self { sky_top: [110, 200, 190], sky_horizon: [225, 250, 235], ground: [200, 232, 214], plate: [60, 90, 86] },
            _ => Self { sky_top: [86, 150, 235], sky_horizon: [205, 228, 250], ground: [222, 226, 232], plate: [58, 62, 74] },
        }
    }
}

fn lin(c: [u8; 3]) -> [f32; 4] {
    let f = |x: u8| (x as f32 / 255.0).powf(2.2);
    [f(c[0]), f(c[1]), f(c[2]), 1.0]
}

pub struct View {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_y_deg: f32,
    /// Bounding sphere of everything that should cast shadows.
    pub scene_center: Vec3,
    pub scene_radius: f32,
    pub ground_y: f32,
    /// Post-projection offset in NDC, to place the subject off-centre (e.g. leave room for a label).
    pub shift: [f32; 2],
}

impl View {
    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(self.shift[0], self.shift[1], 0.0))
            * Mat4::perspective_rh(self.fov_y_deg.to_radians(), aspect, 0.5, 2000.0)
    }
}

pub struct Renderer {
    pub width: u32,
    pub height: u32,
    device: wgpu::Device,
    queue: wgpu::Queue,
    uniform_buf: wgpu::Buffer,
    cube_buf: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    shadow_bg: wgpu::BindGroup,
    main_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    msaa_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    resolve_tex: wgpu::Texture,
    readback: wgpu::Buffer,
    padded_row: u32,
    shadow_pipe: wgpu::RenderPipeline,
    cube_pipe: wgpu::RenderPipeline,
    ground_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    pub theme: Theme,
}

impl Renderer {
    pub fn new(width: u32, height: u32, theme: Theme) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .context("no GPU adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits {
                max_texture_dimension_2d: SHADOW_SIZE.max(width).max(height),
                ..wgpu::Limits::default()
            },
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .context("request_device")?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxreel"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cube_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube"),
            contents: bytemuck::cast_slice(&cube_vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instance_cap = 1 << 16;
        let instance_buf = make_instance_buf(&device, instance_cap);

        let tex = |label, w, h, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let shadow_tex = tex(
            "shadow",
            SHADOW_SIZE,
            SHADOW_SIZE,
            DEPTH_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let shadow_view = shadow_tex.create_view(&Default::default());
        let msaa_view = tex("msaa", width, height, COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT)
            .create_view(&Default::default());
        let depth_view = tex("depth", width, height, DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT)
            .create_view(&Default::default());
        let resolve_tex = tex(
            "resolve",
            width,
            height,
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let padded_row = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded_row * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow"),
            entries: &[uniform_entry],
        });
        let main_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("main"),
            entries: &[
                uniform_entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow"),
            layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("main"),
            layout: &main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });

        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&shadow_bgl],
            push_constant_ranges: &[],
        });
        let main_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&main_bgl],
            push_constant_ranges: &[],
        });

        let vertex_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32];
        let instance_attrs = wgpu::vertex_attr_array![3 => Float32x3, 4 => Unorm8x4, 5 => Float32x3, 6 => Float32, 7 => Uint32x2];
        let cube_buffers = [
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attrs,
            },
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &instance_attrs,
            },
        ];

        let depth_state = |write: bool, compare| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: compare,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let msaa = wgpu::MultisampleState { count: SAMPLES, ..Default::default() };
        let color_target = [Some(wgpu::ColorTargetState {
            format: COLOR_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let primitive = wgpu::PrimitiveState { cull_mode: None, ..Default::default() };

        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                buffers: &cube_buffers,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive,
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
                ..depth_state(true, wgpu::CompareFunction::LessEqual)
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let mk_main = |label, vs, fs, buffers: &[wgpu::VertexBufferLayout], depth| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&main_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &color_target,
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil: Some(depth),
                multisample: msaa,
                multiview: None,
                cache: None,
            })
        };
        let cube_pipe = mk_main("cube", "vs_cube", "fs_cube", &cube_buffers, depth_state(true, wgpu::CompareFunction::Less));
        let ground_pipe = mk_main("ground", "vs_ground", "fs_ground", &[], depth_state(true, wgpu::CompareFunction::Less));
        let sky_pipe = mk_main("sky", "vs_sky", "fs_sky", &[], depth_state(false, wgpu::CompareFunction::Always));

        Ok(Self {
            width,
            height,
            device,
            queue,
            uniform_buf,
            cube_buf,
            instance_buf,
            instance_cap,
            shadow_bg,
            main_bg,
            shadow_view,
            msaa_view,
            depth_view,
            resolve_tex,
            readback,
            padded_row,
            shadow_pipe,
            cube_pipe,
            ground_pipe,
            sky_pipe,
            theme,
        })
    }

    /// Renders one frame into `out` as tightly packed RGBA8 (sRGB), top row first.
    pub fn render(&mut self, instances: &[Instance], view: &View, out: &mut Vec<u8>) -> Result<()> {
        if instances.len() > self.instance_cap {
            self.instance_cap = instances.len().next_power_of_two();
            self.instance_buf = make_instance_buf(&self.device, self.instance_cap);
        }
        if !instances.is_empty() {
            self.queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
        }

        let aspect = self.width as f32 / self.height as f32;
        let proj = view.projection(aspect);
        let view_m = Mat4::look_at_rh(view.eye, view.target, Vec3::Y);
        // Sun from upper-left-front; a fixed light keeps shadows readable while the camera orbits.
        let sun = Vec3::new(-0.45, 0.85, 0.35).normalize();
        let r = view.scene_radius.max(1.0) * 1.15;
        let light_view = Mat4::look_at_rh(view.scene_center + sun * r * 2.0, view.scene_center, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-r, r, -r, r, 0.1, r * 4.0);
        let t = self.theme;
        let dist = view.eye.distance(view.target);
        let u = Uniforms {
            view_proj: (proj * view_m).to_cols_array_2d(),
            inv_view_proj: (proj * view_m).inverse().to_cols_array_2d(),
            light_vp: (light_proj * light_view).to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, 0.0],
            cam: [view.eye.x, view.eye.y, view.eye.z, 0.0],
            sky_top: lin(t.sky_top),
            sky_horizon: lin(t.sky_horizon),
            ground: lin(t.ground),
            params: [dist * 1.2, dist * 3.5, 1.0 / SHADOW_SIZE as f32, view.ground_y],
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));

        let mut enc = self.device.create_command_encoder(&Default::default());
        let n = instances.len() as u32;
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if n > 0 {
                pass.set_pipeline(&self.shadow_pipe);
                pass.set_bind_group(0, &self.shadow_bg, &[]);
                pass.set_vertex_buffer(0, self.cube_buf.slice(..));
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                pass.draw(0..36, 0..n);
            }
        }
        let resolve_view = self.resolve_tex.create_view(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&resolve_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.main_bg, &[]);
            pass.set_pipeline(&self.sky_pipe);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.ground_pipe);
            pass.draw(0..6, 0..1);
            if n > 0 {
                pass.set_pipeline(&self.cube_pipe);
                pass.set_vertex_buffer(0, self.cube_buf.slice(..));
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                pass.draw(0..36, 0..n);
            }
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.resolve_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);

        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::Wait).context("device poll")?;
        {
            let data = slice.get_mapped_range();
            let row = (self.width * 4) as usize;
            out.clear();
            out.reserve(row * self.height as usize);
            for y in 0..self.height as usize {
                let start = y * self.padded_row as usize;
                out.extend_from_slice(&data[start..start + row]);
            }
        }
        self.readback.unmap();
        Ok(())
    }
}

fn make_instance_buf(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instances"),
        size: (cap * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
