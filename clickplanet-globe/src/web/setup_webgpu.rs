//! wgpu setup and rendering: WebGPU when the browser supports it, otherwise WebGL2 through wgpu's
//! GL backend. Works on an `OffscreenCanvas` (globe in a worker) or an `HTMLCanvasElement` (main
//! thread fallback).

use clickplanet_globe_protocol::RendererPreference;
use web_sys::{HtmlCanvasElement, ImageBitmap, OffscreenCanvas};
use wgpu::util::DeviceExt;

use super::create_sphere::{create_sphere, Vertex};
use crate::core::controls::CameraFrame;
use crate::core::tiles::DirtyUpload;

/// Diameter of a tile in world units. The original drew points of `zoom * 1.5 * height / 1000`
/// pixels with an orthographic camera of half height `1 / zoom`, i.e. a constant 0.003 world units.
pub const TILE_DIAMETER: f32 = 0.003;
/// Size of the country region table in the shader.
pub const MAX_COUNTRIES: usize = 512;

pub enum CanvasTarget {
    Offscreen(OffscreenCanvas),
    Html(HtmlCanvasElement),
}

impl CanvasTarget {
    fn set_size(&self, width: u32, height: u32) {
        match self {
            CanvasTarget::Offscreen(c) => {
                c.set_width(width);
                c.set_height(height);
            }
            CanvasTarget::Html(c) => {
                c.set_width(width);
                c.set_height(height);
            }
        }
    }

    fn surface_target(&self) -> wgpu::SurfaceTarget<'static> {
        match self {
            CanvasTarget::Offscreen(c) => wgpu::SurfaceTarget::OffscreenCanvas(c.clone()),
            CanvasTarget::Html(c) => wgpu::SurfaceTarget::Canvas(c.clone()),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    view_proj: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
    params: [f32; 4],
    hovered: [u32; 4],
}

/// Device, queue and configured surface.
pub struct GpuContext {
    canvas: CanvasTarget,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub backend_name: String,
    pub max_texture_dimension: u32,
}

impl GpuContext {
    pub async fn new(canvas: CanvasTarget, preference: RendererPreference, width: u32, height: u32) -> Result<Self, String> {
        let backends = match preference {
            RendererPreference::Auto => wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            RendererPreference::WebGpu => wgpu::Backends::BROWSER_WEBGPU,
            RendererPreference::WebGl2 => wgpu::Backends::GL,
        };
        let instance = wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        })
        .await;

        canvas.set_size(width.max(1), height.max(1));
        let surface = instance
            .create_surface(canvas.surface_target())
            .map_err(|e| format!("cannot create a surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("no graphics adapter ({preference:?}): {e}"))?;
        let info = adapter.get_info();
        let backend_name = match info.backend {
            wgpu::Backend::BrowserWebGpu => "WebGPU".to_string(),
            wgpu::Backend::Gl => "WebGL2".to_string(),
            other => format!("{other:?}"),
        };

        // WebGL2-compatible limits, raised to what the adapter supports (texture size matters for
        // the earth texture).
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("globe device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits.clone(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
            })
            .await
            .map_err(|e| format!("cannot create a device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(GpuContext {
            canvas,
            device,
            queue,
            surface,
            config,
            backend_name,
            max_texture_dimension: limits.max_texture_dimension_2d,
        })
    }

    /// Texture format matching the surface, so colours are passed through unchanged like in the
    /// original WebGL renderer.
    fn color_texture_format(&self) -> wgpu::TextureFormat {
        if self.config.format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }

    /// Uploads browser-decoded bitmaps as the mip chain of a texture.
    fn texture_from_bitmaps(&self, label: &str, mips: &[ImageBitmap]) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width: mips[0].width(),
            height: mips[0].height(),
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_texture_format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        for (level, bitmap) in mips.iter().enumerate() {
            self.queue.copy_external_image_to_texture(
                &wgpu::CopyExternalImageSourceInfo {
                    source: wgpu::ExternalImageSource::ImageBitmap(bitmap.clone()),
                    origin: wgpu::Origin2d::ZERO,
                    flip_y: false,
                },
                wgpu::CopyExternalImageDestInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                    color_space: wgpu::PredefinedColorSpace::Srgb,
                    premultiplied_alpha: false,
                },
                wgpu::Extent3d {
                    width: bitmap.width(),
                    height: bitmap.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
        texture
    }
}

pub struct Renderer {
    gpu: GpuContext,
    depth_view: wgpu::TextureView,
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertices: wgpu::Buffer,
    sphere_indices: wgpu::Buffer,
    sphere_index_count: u32,
    tile_pipeline: wgpu::RenderPipeline,
    tile_positions: wgpu::Buffer,
    tile_owners: wgpu::Buffer,
    tile_count: u32,
    frame_uniforms: wgpu::Buffer,
    regions: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    atlas_size: [f32; 2],
}

fn depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

impl Renderer {
    pub fn new(gpu: GpuContext, positions: &[f32], earth_mips: &[ImageBitmap], atlas: &ImageBitmap) -> Self {
        let device = &gpu.device;

        let (vertices, indices) = create_sphere(0.999, 64, 128);
        let sphere_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let sphere_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let tile_count = (positions.len() / 3) as u32;
        let tile_positions = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile positions"),
            contents: bytemuck::cast_slice(&positions[..tile_count as usize * 3]),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let tile_owners = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile owners"),
            contents: bytemuck::cast_slice(&vec![0u32; tile_count as usize]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let frame_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let regions = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("country regions"),
            contents: bytemuck::cast_slice(&vec![[0.0f32; 4]; MAX_COUNTRIES]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let earth = gpu.texture_from_bitmaps("earth", earth_mips);
        let atlas_texture = gpu.texture_from_bitmaps("flag atlas", std::slice::from_ref(atlas));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_entry = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D2,
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globe"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                uniform_entry(1, wgpu::ShaderStages::VERTEX),
                texture_entry(2),
                texture_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let earth_view = earth.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: regions.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&earth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let depth_state = |write| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: write,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let earth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("earth"),
            source: wgpu::ShaderSource::Wgsl(include_str!("earth_textured.wgsl").into()),
        });
        let sphere_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("earth"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &earth_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &earth_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(true)),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let tile_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tiles"),
            source: wgpu::ShaderSource::Wgsl(include_str!("tiles.wgsl").into()),
        });
        let tile_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tiles"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tile_shader,
                entry_point: Some("vs_tile"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 12,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![1 => Uint32],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &tile_shader,
                entry_point: Some("fs_tile"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(false)),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let depth_view = depth_view(device, gpu.config.width, gpu.config.height);
        Renderer {
            sphere_index_count: indices.len() as u32,
            atlas_size: [atlas.width() as f32, atlas.height() as f32],
            gpu,
            depth_view,
            sphere_pipeline,
            sphere_vertices,
            sphere_indices,
            tile_pipeline,
            tile_positions,
            tile_owners,
            tile_count,
            frame_uniforms,
            regions,
            bind_group,
        }
    }

    pub fn backend_name(&self) -> &str {
        &self.gpu.backend_name
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if width == self.gpu.config.width && height == self.gpu.config.height {
            return;
        }
        self.gpu.canvas.set_size(width, height);
        self.gpu.config.width = width;
        self.gpu.config.height = height;
        self.gpu.surface.configure(&self.gpu.device, &self.gpu.config);
        self.depth_view = depth_view(&self.gpu.device, width, height);
    }

    /// Country index → atlas region table.
    pub fn set_regions(&self, regions: &[[f32; 4]]) {
        let mut table = vec![[0.0f32; 4]; MAX_COUNTRIES];
        for (slot, region) in table.iter_mut().zip(regions) {
            *slot = *region;
        }
        self.gpu.queue.write_buffer(&self.regions, 0, bytemuck::cast_slice(&table));
    }

    /// Partial update of the owner instance buffer.
    pub fn upload_owners(&self, owners: &[u32], dirty: &DirtyUpload) {
        if let DirtyUpload::Ranges(ranges) = dirty {
            for range in ranges {
                let (start, end) = (range.start as usize, (range.end as usize).min(owners.len()));
                if start < end {
                    self.gpu.queue.write_buffer(
                        &self.tile_owners,
                        start as u64 * 4,
                        bytemuck::cast_slice(&owners[start..end]),
                    );
                }
            }
        }
    }

    pub fn render(&mut self, camera: &CameraFrame, hovered: Option<u32>) {
        let uniforms = FrameUniforms {
            view_proj: camera.view_proj.to_cols_array_2d(),
            right: camera.right.extend(0.0).to_array(),
            up: camera.up.extend(0.0).to_array(),
            params: [TILE_DIAMETER, self.atlas_size[0], self.atlas_size[1], 0.0],
            hovered: [hovered.unwrap_or(0), 0, 0, 0],
        };
        self.gpu.queue.write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(&uniforms));

        let output = match self.gpu.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.gpu.surface.configure(&self.gpu.device, &self.gpu.config);
                return;
            }
            Err(e) => {
                log::warn!("cannot get the next frame: {e:?}");
                return;
            }
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("globe"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);

            pass.set_pipeline(&self.sphere_pipeline);
            pass.set_vertex_buffer(0, self.sphere_vertices.slice(..));
            pass.set_index_buffer(self.sphere_indices.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.sphere_index_count, 0, 0..1);

            pass.set_pipeline(&self.tile_pipeline);
            pass.set_vertex_buffer(0, self.tile_positions.slice(..));
            pass.set_vertex_buffer(1, self.tile_owners.slice(..));
            pass.draw(0..4, 0..self.tile_count);
        }
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

/// Earth texture width for this device: the adapter limit, capped to 8192 (4096 on small
/// screens, to save memory on phones).
pub fn earth_texture_width(max_texture_dimension: u32, css_min_side: f32) -> u32 {
    let cap = if css_min_side < 800.0 { 4096 } else { 8192 };
    max_texture_dimension.min(cap)
}
