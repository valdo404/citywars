use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement};
use gloo_utils::document;
use std::cell::RefCell;

use super::bindings::*;

// Use a struct to hold the animation state to avoid thread_local borrowing issues
struct ThreeJsState {
    scene: Option<Scene>,
    camera: Option<PerspectiveCamera>,
    renderer: Option<WebGLRenderer>,
    cube: Option<Mesh>,
    animation_frame: Option<i32>,
}

// Global state for ThreeJS objects
thread_local! {
    static STATE: RefCell<ThreeJsState> = RefCell::new(ThreeJsState {
        scene: None,
        camera: None,
        renderer: None,
        cube: None,
        animation_frame: None,
    });
    static CALLBACKS: RefCell<Vec<Closure<dyn FnMut()>>> = RefCell::new(Vec::new());
}

/// Initialize a simple Three.js scene with a spinning cube
pub fn init_cube(canvas_id: &str) -> Result<(), JsError> {
    // Clean up any previous instance
    cleanup_cube();

    // Get the canvas element
    let document = document();
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsError::new(&format!("No canvas found with id: {}", canvas_id)))?;
    
    let canvas: HtmlCanvasElement = canvas
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsError::new("Element is not a canvas"))?;
    
    // Create scene
    let scene = Scene::new();
    
    // Set up camera
    let window = web_sys::window().ok_or_else(|| JsError::new("No window found"))?;
    let width = window.inner_width().unwrap().as_f64().unwrap();
    let height = window.inner_height().unwrap().as_f64().unwrap();
    let aspect = width / height;
    
    let camera = PerspectiveCamera::new(75.0, aspect, 0.1, 1000.0);
    camera.position().set_z(5.0);
    
    // Create renderer
    let renderer_params = js_sys::Object::new();
    js_sys::Reflect::set(&renderer_params, &JsValue::from_str("canvas"), &canvas)
        .map_err(|_| JsError::new("Failed to set canvas parameter"))?;
    
    let renderer = WebGLRenderer::new_with_parameters(&renderer_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    
    // Create a cube
    let geometry = BoxGeometry::new(1.0, 1.0, 1.0);
    // Create material with green color (0x00ff00 = 65280 in decimal)
    let material_params = js_sys::Object::new();
    js_sys::Reflect::set(&material_params, &JsValue::from_str("color"), &JsValue::from_f64(65280.0))
        .map_err(|_| JsError::new("Failed to set material color"))?;
    
    let material = MeshBasicMaterial::new_with_params(&material_params);
    
    // Convert types implicitly using JsValue for JavaScript interop
    let geom_js: &JsValue = geometry.as_ref();
    let mat_js: &JsValue = material.as_ref();
    
    // Create the mesh with the geometry and material
    let cube = Mesh::new_with_geometry_material(
        &geom_js.clone().into(),  // Convert to the expected BufferGeometry type
        &mat_js.clone().into(),    // Convert to the expected Material type
    );
    
    // Add cube to scene
    scene.add(&cube);
    
    // Store objects in our state struct
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.scene = Some(scene);
        state.camera = Some(camera);
        state.renderer = Some(renderer);
        state.cube = Some(cube);
    });
    
    // Start animation loop
    start_animation()?;
    
    // Cube initialization complete
    Ok(())
}

/// Start the animation loop for the cube
fn start_animation() -> Result<(), JsError> {
    // Create animation callback
    let callback = Closure::wrap(Box::new(move || {
        // Use a safer approach by getting each piece of state individually
        STATE.with(|state_ref| {
            // First, check if everything is initialized
            let is_initialized = {
                let state = state_ref.borrow();
                state.scene.is_some() && state.camera.is_some() && 
                state.renderer.is_some() && state.cube.is_some()
            };
            
            if !is_initialized {
                return;
            }
            
            // Handle cube rotation - separate borrow
            {
                let state = state_ref.borrow();
                if let Some(cube) = &state.cube {
                    // Get the current rotation values
                    let current_x = cube.rotation().x();
                    let current_y = cube.rotation().y();
                    
                    // Update rotation
                    cube.rotation().set_x(current_x + 0.01);
                    cube.rotation().set_y(current_y + 0.01);
                }
            }
            
            // Render the scene - separate borrow
            {
                let state = state_ref.borrow();
                if let (Some(scene), Some(camera), Some(renderer)) = (
                    &state.scene, &state.camera, &state.renderer) {
                    // Convert camera to JsValue for the render method
                    let camera_js: &JsValue = camera.as_ref();
                    renderer.render(scene, camera_js);
                }
            }
            
            // Request next animation frame - separate borrow
            CALLBACKS.with(|callbacks_ref| {
                let callback = callbacks_ref.borrow();
                if let Some(callback) = callback.get(0) {
                    let id = request_animation_frame(callback);
                    // Update animation frame ID with a new borrow
                    let mut state = state_ref.borrow_mut();
                    state.animation_frame = Some(id);
                }
            });
        });
    }) as Box<dyn FnMut()>);
    
    // Store callback to keep it alive
    CALLBACKS.with(|callbacks_ref| {
        callbacks_ref.borrow_mut().push(callback);
    });
    
    // Request first animation frame
    CALLBACKS.with(|callbacks_ref| {
        let callback = callbacks_ref.borrow();
        if let Some(callback) = callback.get(0) {
            let id = request_animation_frame(callback);
            STATE.with(|state_ref| {
                let mut state = state_ref.borrow_mut();
                state.animation_frame = Some(id);
            });
        }
    });
    
    Ok(())
}

/// Helper function to request animation frame
fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
    let window = web_sys::window().expect("no global window exists");
    window
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK")
}

/// Clean up Three.js resources
pub fn cleanup_cube() {
    // Cancel animation frame and clean up all resources in a single borrow
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        
        // Cancel animation frame
        if let Some(id) = state.animation_frame {
            if let Some(window) = web_sys::window() {
                window.cancel_animation_frame(id).ok();
            }
        }
        
        // Dispose of renderer if needed
        if let Some(renderer) = &state.renderer {
            renderer.dispose();
        }
        
        // Reset all state
        state.animation_frame = None;
        state.scene = None;
        state.camera = None;
        state.renderer = None;
        state.cube = None;
    });
    
    // Clear callbacks
    CALLBACKS.with(|callbacks_ref| {
        callbacks_ref.borrow_mut().clear();
    });
}
