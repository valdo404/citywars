use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use log::{debug, info};
use wasm_bindgen_futures::spawn_local;
use glam::{Mat4, Vec3};

use crate::app::components::earth::setup_webgpu::{Uniforms, WebGpuContext};

#[allow(dead_code)]
pub struct RenderLoopContext {
    pub webgpu_context: WebGpuContext,
    pub rotation: Signal<f32>,
}

#[allow(dead_code)]
pub fn start_animation(context: RenderLoopContext) {
    info!("Starting WebGPU animation loop");
    let WebGpuContext { 
        device, 
        queue, 
        surface, 
        render_pipeline, 
        bind_group, 
        vertex_buffer, 
        index_buffer, 
        uniform_buffer, 
        depth_texture, 
        num_indices, 
        config,
    } = context.webgpu_context;
    let rotation = context.rotation;
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    // Extract dimensions
    let width = config.width;
    let height = config.height;
    
    spawn_local(async move {
        loop {
            let output = match surface.get_current_texture() {
                Ok(texture) => texture,
                Err(err) => {
                    debug!("Failed to get current texture: {:?}", err);
                    continue;
                }
            };
            
            let texture_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
            
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
            
            let rotation_angle = rotation();
            
            let model = Mat4::from_rotation_y(rotation_angle);
            let view_matrix = Mat4::look_at_rh(
                Vec3::new(0.0, 0.0, 2.5), // camera position
                Vec3::new(0.0, 0.0, 0.0), // target
                Vec3::new(0.0, 1.0, 0.0), // up
            );
            let aspect = width as f32 / height as f32;
            let projection = Mat4::perspective_rh(
                45.0f32.to_radians(),
                aspect,
                0.1,
                100.0,
            );
            
            let mvp = projection * view_matrix * model;
            
            // Update uniform buffer
            queue.write_buffer(
                &uniform_buffer,
                0,
                bytemuck::cast_slice(&[Uniforms { mvp: mvp.to_cols_array_2d() }]),
            );
            
            // Begin render pass
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
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
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                
                render_pass.set_pipeline(&render_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..num_indices, 0, 0..1);
            }
            
            queue.submit(std::iter::once(encoder.finish()));
            output.present();
            
            TimeoutFuture::new(16).await;
        }
    });
    
    info!("Started animation loop");
}
