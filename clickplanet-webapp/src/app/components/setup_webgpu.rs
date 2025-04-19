use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::SurfaceTarget;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3, Quat};
use std::cell::RefCell;
use std::rc::Rc;
use std::f32::consts::PI;
use log::{info, debug, error};
use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::components::create_sphere::{Vertex, create_sphere};

/// Uniform buffer data for transformations
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub mvp: [[f32; 4]; 4],
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
pub async fn setup_webgpu(canvas: HtmlCanvasElement, rotation: Signal<f32>) {
    info!("Setting up WebGPU for globe rendering");
    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    
    // Create WGPU instance
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        backend_options: Default::default(),
    });
    
    debug!("Created WebGPU instance");
    
    // For WebGPU in the browser, we need to create a surface from the canvas
    // In wgpu 25.0, we use SurfaceTarget::Canvas
    let surface_target = SurfaceTarget::Canvas(canvas.clone());
    let surface = instance
        .create_surface(surface_target)
        .expect("Failed to create surface from canvas");
        
    // Request adapter
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }).await.expect("Failed to find appropriate adapter");
        
    // Create device and queue
    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Globe Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::default(),
        }
    ).await.expect("Failed to create device");
    
    info!("Created device and queue");
        
    // Get preferred format
    let capabilities = surface.get_capabilities(&adapter);
    let surface_format = capabilities.formats[0];
        
    // Configure the surface
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
        
    // Create Earth sphere mesh with proper dimensions
    let (vertices, indices) = create_sphere(1.0, 64, 128);
        
    // Create vertex buffer
    let vertex_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }
    );
    
    // Create index buffer
    let index_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }
    );
    
    let num_indices = indices.len() as u32;
    
    info!("Created mesh with {} vertices and {} indices", vertices.len(), indices.len());
        
    // Create uniform buffer
    let uniform_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                mvp: Mat4::IDENTITY.to_cols_array_2d(),
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }
    );
    
    info!("Created uniform buffer");
        
    // Create bind group layout
    let bind_group_layout = device.create_bind_group_layout(
        &wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        }
    );
        
    // Create bind group
    let bind_group = device.create_bind_group(
        &wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        }
    );
        
    // Create shader module
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Globe Shader"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("globe.wgsl"))),
    });
        
    // Create render pipeline layout
    let pipeline_layout = device.create_pipeline_layout(
        &wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        }
    );
        
    // Create render pipeline
    let render_pipeline = device.create_render_pipeline(
        &wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        }
    );
    
    info!("Created render pipeline");
        
    // Create depth texture
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    // Start animation loop
    spawn_local(async move {
        loop {
            // Get current rotation
            let current_rotation = rotation();
            
            // Create model-view-projection matrix
            let model = Mat4::from_rotation_y(current_rotation);
            let view = Mat4::look_at_rh(
                Vec3::new(0.0, 0.0, 3.0),  // Camera position
                Vec3::new(0.0, 0.0, 0.0),  // Look at center
                Vec3::new(0.0, 1.0, 0.0),  // Up vector
            );
            let aspect = width as f32 / height as f32;
            let proj = Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 100.0);
            
            let mvp = proj * view * model;
            
            // Update uniform buffer
            queue.write_buffer(
                &uniform_buffer,
                0,
                bytemuck::cast_slice(&[Uniforms {
                    mvp: mvp.to_cols_array_2d(),
                }]),
            );
            
            // Get a frame
            match surface.get_current_texture() {
                Ok(frame) => {
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    
                    // Create command encoder
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });
                    
                    // Create render pass
                    {
                        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Main Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.05, 
                                        g: 0.1, 
                                        b: 0.2, 
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: &depth_view,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.0),
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }),
                            occlusion_query_set: None,
                            timestamp_writes: None,
                        });
                        
                        // Set pipeline and bind groups
                        render_pass.set_pipeline(&render_pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                        
                        // Draw the sphere
                        render_pass.draw_indexed(0..num_indices, 0, 0..1);
                    }
                    
                    // Submit commands
                    queue.submit(std::iter::once(encoder.finish()));
                    frame.present();
                },
                Err(e) => {
                    error!("Failed to get current texture: {:?}", e);
                }
            }
            
            // Yield to browser to avoid blocking the main thread
            gloo_timers::future::TimeoutFuture::new(16).await; // ~60fps
        }
    });
    
    info!("Started animation loop");
}
