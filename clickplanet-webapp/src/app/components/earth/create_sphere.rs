use std::f32::consts::PI;
use bytemuck::{Pod, Zeroable};

/// Vertex data for the globe mesh
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// Create a UV sphere with the given radius and segment counts
pub fn create_sphere(radius: f32, latitude_segments: u32, longitude_segments: u32) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Generate vertices
    for lat in 0..=latitude_segments {
        let theta = lat as f32 * PI / latitude_segments as f32;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();
        
        for lon in 0..=longitude_segments {
            let phi = lon as f32 * 2.0 * PI / longitude_segments as f32;
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();
            
            // Calculate vertex position
            let x = cos_phi * sin_theta;
            let y = cos_theta;
            let z = sin_phi * sin_theta;
            
            // Position multiplied by radius
            let position = [x * radius, y * radius, z * radius];
            
            // Normal is just the normalized position for a sphere
            let normal = [x, y, z];
            
            vertices.push(Vertex { position, normal });
        }
    }
    
    // Generate indices for triangle strips
    for lat in 0..latitude_segments {
        for lon in 0..longitude_segments {
            let first = lat * (longitude_segments + 1) + lon;
            let second = first + longitude_segments + 1;
            
            // First triangle
            indices.push(first as u16);
            indices.push(second as u16);
            indices.push((first + 1) as u16);
            
            // Second triangle
            indices.push(second as u16);
            indices.push((second + 1) as u16);
            indices.push((first + 1) as u16);
        }
    }
    
    (vertices, indices)
}
