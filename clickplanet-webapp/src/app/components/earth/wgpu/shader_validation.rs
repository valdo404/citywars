
/// Validates WGSL shader code by attempting to compile it
#[cfg(test)]
pub fn validate_shader_code(shader_source: &str) {
    use wgpu::ShaderSource;
    
    // This function ensures the shader code is syntactically valid
    // by attempting to create a shader module with it.
    // It will panic if the shader has syntax errors.
    
    // Create a headless device for testing
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter for shader testing");

        let (device, _) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Test Device"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::default(),
                },
            )
            .await
            .expect("Failed to create device for shader testing");

        // Create shader module - this will validate the syntax
        let _shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Test Shader"),
            source: ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_source)),
        });
        
        // If we reach here without panicking, the shader is valid
        println!("WGSL shader validation successful");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_globe_shader_validation() {
        // This test validates that the globe.wgsl shader has valid syntax
        validate_shader_code(include_str!("globe.wgsl"));
    }
}
