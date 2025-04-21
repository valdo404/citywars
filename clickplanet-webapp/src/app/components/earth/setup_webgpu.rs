use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;
use wgpu::SurfaceTarget;
use glam::Mat4;
use log::{debug, info};
use dioxus::prelude::*;
use bytemuck::{Pod, Zeroable, cast_slice};
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use image::{GenericImageView, imageops::FilterType};

use crate::app::components::earth::create_sphere::{Vertex, create_sphere};
use crate::app::components::earth::animation_loop::{start_animation, RenderLoopContext};

#[cfg(test)]
use crate::app::components::earth::shader_validation::validate_shader_code;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub mvp: [[f32; 4]; 4],
}

/// WebGPU rendering context containing all necessary components
pub struct WebGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub render_pipeline: wgpu::RenderPipeline,
    pub depth_texture: wgpu::Texture,
    pub num_indices: u32,
    pub config: wgpu::SurfaceConfiguration,
}

/// Create a signal-based animation state for the globe
pub fn use_animation_state() -> Signal<f32> {
    let rotation = use_signal(|| 0.0f32);
    
    use_effect(move || {
        let mut rotation_state = rotation.clone();
        spawn_local(async move {
            loop {
                // Update rotation (in radians)
                rotation_state.set(rotation_state() + 0.005);
                // Yield to browser to avoid blocking the main thread
                gloo_timers::future::TimeoutFuture::new(16).await; // ~60fps
            }
        });
    });
    
    rotation
}

/// Set up WebGPU and start the rendering loop
pub async fn setup_scene(canvas: HtmlCanvasElement, rotation: Signal<f32>) {
    info!("Setting up WebGPU for globe rendering");
    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;

    // Create WGPU instance first
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        backend_options: Default::default(),
    });
        
    debug!("Created WebGPU instance");

    // Create surface from canvas for WebGPU
    let surface = instance
        .create_surface(SurfaceTarget::Canvas(canvas.clone()))
        .expect("Failed to create surface from canvas");

    // Initialize full WebGPU context (adapter, device, surface config, pipeline, resources)
    let context = create_render_pipeline(&instance, surface, width, height).await;

    // Start animation using TS-like startAnimation
    start_animation(RenderLoopContext { webgpu_context: context, rotation });
    info!("Started animation loop");
}

/// Create WebGPU context: adapter, device, surface config, pipeline and resources
async fn create_render_pipeline(
    instance: &wgpu::Instance,
    surface: wgpu::Surface<'static>,
    width: u32,
    height: u32,
) -> WebGpuContext {
    // Request adapter and create device/queue
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find appropriate adapter");
    // Request device and queue
    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Globe Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::default(),
        },
    )
    .await
    .expect("Failed to create device");
    info!("Created adapter, device and queue");

    // Configure the surface
    let capabilities = surface.get_capabilities(&adapter);
    let surface_format = capabilities.formats[0];
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Create render pipeline and resources
    // Mesh generation
    let (vertices, indices) = create_sphere(1.0, 64, 128);
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let num_indices = indices.len() as u32;
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[Uniforms { mvp: Mat4::IDENTITY.to_cols_array_2d() }]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            // earth texture binding
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { multisampled: false, view_dimension: wgpu::TextureViewDimension::D2, sample_type: wgpu::TextureSampleType::Float { filterable: true } },
                count: None,
            },
            // sampler binding
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    // Fetch and downscale earth image via asset API
    let image_url = format!("{}/earth/3_no_ice_clouds_16k.jpg", env!("CITYWARS_STATIC_SITE"));
    let resp = Request::get(&image_url)
        .send().await.expect("Failed to fetch earth image");
    let data = resp.binary().await.expect("Failed to read earth image bytes");
    let mut img = image::load_from_memory(&data).expect("Failed to decode earth image");
    let max_dim = device.limits().max_texture_dimension_2d;
    let (w, h) = img.dimensions();
    if w > max_dim || h > max_dim {
        let aspect = w as f32 / h as f32;
        let (new_w, new_h) = if w > h {
            (max_dim, (max_dim as f32 / aspect) as u32)
        } else {
            ((max_dim as f32 * aspect) as u32, max_dim)
        };
        img = img.resize(new_w, new_h, FilterType::Triangle);
    }
    let rgba = img.to_rgba8();
    let (tex_w, tex_h) = img.dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Earth Texture"),
        size: wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: Default::default(), aspect: wgpu::TextureAspect::All },
        &rgba,
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * tex_w), rows_per_image: Some(tex_h) },
        wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
    );
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Earth Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Textured Earth Shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("earth_textured.wgsl"))),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        cache: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState { format: surface_format, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth24Plus, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::LessEqual, stencil: wgpu::StencilState::default(), bias: wgpu::DepthBiasState::default() }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    WebGpuContext { device, queue, surface, vertex_buffer, index_buffer, uniform_buffer, bind_group, render_pipeline, depth_texture, num_indices, config }
}
