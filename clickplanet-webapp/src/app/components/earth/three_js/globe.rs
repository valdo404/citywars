use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, window};
use gloo_utils::document;
use std::rc::Rc;
use std::cell::RefCell;
use log::info;

use super::bindings::*;

// Global state for ThreeJS objects
thread_local! {
    static SCENE: RefCell<Option<Scene>> = RefCell::new(None);
    static CAMERA: RefCell<Option<OrthographicCamera>> = RefCell::new(None);
    static RENDERER: RefCell<Option<WebGLRenderer>> = RefCell::new(None);
    static CONTROLS: RefCell<Option<OrbitControls>> = RefCell::new(None);
    static EARTH_MESH: RefCell<Option<Mesh>> = RefCell::new(None);
    static ANIMATION_FRAME: RefCell<Option<i32>> = RefCell::new(None);
    static CALLBACKS: RefCell<Vec<Closure<dyn FnMut()>>> = RefCell::new(Vec::new());
}

/// Initialize the Three.js scene with the Earth globe
pub fn init_globe(canvas_id: &str) -> Result<(), JsError> {
    // Set the static site URL as a global variable for texture loading
    let static_site = env!("CITYWARS_STATIC_SITE");
    let window = web_sys::window().ok_or_else(|| JsError::new("No window found"))?;
    
    // Check if THREE object is available in the global scope
    let three_available = js_sys::Reflect::has(&window, &JsValue::from_str("THREE"))
        .map_err(|_| JsError::new("Failed to check for THREE global object"))?;
    
    if !three_available {
        return Err(JsError::new("THREE global object not found. Make sure Three.js is loaded properly."));
    }
    
    // Set the static site URL directly on the window object
    js_sys::Reflect::set(
        &window,
        &JsValue::from_str("CITYWARS_STATIC_SITE"),
        &JsValue::from_str(static_site)
    ).map_err(|_| JsError::new("Failed to set static site URL"))?;
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
    let web_window = web_sys::window().ok_or_else(|| JsError::new("No window found"))?;
    let width = web_window.inner_width().unwrap().as_f64().unwrap();
    let height = web_window.inner_height().unwrap().as_f64().unwrap();
    let aspect = width / height;
    let camera_size = 1.0;
    
    let camera = OrthographicCamera::new(
        -camera_size * aspect,
        camera_size * aspect,
        camera_size,
        -camera_size,
        0.01,
        100.0
    );
    
    // Position camera
    let position = camera.position();
    position.set_z(5.0);
    
    // Create renderer with canvas
    let renderer_params = js_sys::Object::new();
    js_sys::Reflect::set(&renderer_params, &"canvas".into(), &canvas).unwrap();
    js_sys::Reflect::set(&renderer_params, &"antialias".into(), &JsValue::from_bool(true)).unwrap();
    
    let renderer = WebGLRenderer::new_with_parameters(&renderer_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    
    // Add lighting
    let light = AmbientLight::new(0xffffff, 2.0);
    scene.add(&light);
    
    // Create earth sphere
    let static_site = env!("CITYWARS_STATIC_SITE");
    let texture_loader = TextureLoader::new();
    let earth_texture = texture_loader.load(&format!("{}/earth/3_no_ice_clouds_16k.jpg", static_site));
    
    let material_params = js_sys::Object::new();
    js_sys::Reflect::set(&material_params, &"map".into(), &earth_texture).unwrap();
    
    let earth_geometry = IcosahedronGeometry::new(0.999, 50);
    let earth_material = MeshStandardMaterial::new_with_params(&material_params);
    // Cast to appropriate types for function call
    let geometry_js_value: &JsValue = earth_geometry.as_ref();
    let material_js_value: &JsValue = earth_material.as_ref();
    let buffer_geometry = geometry_js_value.unchecked_ref::<BufferGeometry>();
    let material = material_js_value.unchecked_ref::<Material>();
    let earth_mesh = Mesh::new_with_geometry_material(buffer_geometry, material);
    
    scene.add(&earth_mesh);
    
    // Set up orbit controls
    // Create OrbitControls instance directly using the constructor binding
    let controls = OrbitControls::new(&camera, &renderer.domElement());
    controls.set_min_zoom(1.0);
    controls.set_max_zoom(50.0);
    controls.set_pan_speed(0.1);
    controls.set_enable_damping(true);
    controls.set_auto_rotate(true);
    controls.set_auto_rotate_speed(0.5);
    
    // Add handler for orbit controls changes
    let control_change_callback = Closure::wrap(Box::new(move || {
        CONTROLS.with(|controls_ref| {
            if let Some(controls) = &*controls_ref.borrow() {
                CAMERA.with(|camera_ref| {
                    if let Some(camera) = &*camera_ref.borrow() {
                        let zoom = camera.zoom();
                        controls.set_auto_rotate(zoom == 1.0);
                        controls.set_rotate_speed((1.0 / zoom) / 1.5);
                    }
                });
            }
        });
    }) as Box<dyn FnMut()>);
    
    controls.add_event_listener("change", &control_change_callback);
    
    // Store callback to prevent it from being dropped
    CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().push(control_change_callback);
    });
    
    // Handle window resize
    let resize_callback = Closure::wrap(Box::new(move || {
        let web_window = web_sys::window().unwrap();
        let width = web_window.inner_width().unwrap().as_f64().unwrap();
        let height = web_window.inner_height().unwrap().as_f64().unwrap();
        let aspect = width / height;
        let camera_size = 1.0;
        
        CAMERA.with(|camera_ref| {
            if let Some(camera) = &*camera_ref.borrow() {
                // Update camera aspect ratio
                camera.set_left(-camera_size * aspect);
                camera.set_right(camera_size * aspect);
                camera.update_projection_matrix();
            }
        });
        
        RENDERER.with(|renderer_ref| {
            if let Some(renderer) = &*renderer_ref.borrow() {
                renderer.set_size(width, height);
            }
        });
    }) as Box<dyn FnMut()>);
    
    let web_window = web_sys::window().ok_or_else(|| JsError::new("No window found"))?;
    web_window.add_event_listener_with_callback("resize", resize_callback.as_ref().unchecked_ref())
        .map_err(|_| JsError::new("Failed to add resize event listener"))?;
    
    // Store callback to prevent it from being dropped
    CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().push(resize_callback);
    });
    
    // Store the objects
    SCENE.with(|scene_ref| {
        *scene_ref.borrow_mut() = Some(scene);
    });
    
    CAMERA.with(|camera_ref| {
        *camera_ref.borrow_mut() = Some(camera);
    });
    
    RENDERER.with(|renderer_ref| {
        *renderer_ref.borrow_mut() = Some(renderer);
    });
    
    CONTROLS.with(|controls_ref| {
        *controls_ref.borrow_mut() = Some(controls);
    });
    
    EARTH_MESH.with(|earth_mesh_ref| {
        *earth_mesh_ref.borrow_mut() = Some(earth_mesh);
    });
    
    // Start animation loop
    start_animation();
    
    info!("Three.js Earth globe initialized");
    Ok(())
}

// Animation loop
fn start_animation() {
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // Request next frame
        request_animation_frame(f.borrow().as_ref().unwrap());
        
        // Update controls
        CONTROLS.with(|controls_ref| {
            if let Some(controls) = &*controls_ref.borrow() {
                controls.update();
            }
        });
        
        // Render scene
        SCENE.with(|scene_ref| {
            if let Some(scene) = &*scene_ref.borrow() {
                CAMERA.with(|camera_ref| {
                    if let Some(camera) = &*camera_ref.borrow() {
                        RENDERER.with(|renderer_ref| {
                            if let Some(renderer) = &*renderer_ref.borrow() {
                                renderer.render(scene, camera);
                            }
                        });
                    }
                });
            }
        });
    }) as Box<dyn FnMut()>));
    
    // Start the first frame
    request_animation_frame(g.borrow().as_ref().unwrap());
}

// Helper function to request animation frame
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    let web_window = web_sys::window().unwrap();
    let id = web_window
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Failed to request animation frame");
    
    ANIMATION_FRAME.with(|frame_ref| {
        *frame_ref.borrow_mut() = Some(id);
    });
}

/// Clean up Three.js resources
pub fn cleanup_globe() {
    // Cancel animation frame
    ANIMATION_FRAME.with(|frame_ref| {
        if let Some(id) = *frame_ref.borrow() {
            let web_window = web_sys::window().unwrap();
            web_window.cancel_animation_frame(id).unwrap();
            *frame_ref.borrow_mut() = None;
        }
    });
    
    // Dispose renderer
    RENDERER.with(|renderer_ref| {
        if let Some(renderer) = &*renderer_ref.borrow() {
            renderer.dispose();
            *renderer_ref.borrow_mut() = None;
        }
    });
    
    // Clear references to other objects
    SCENE.with(|scene_ref| {
        *scene_ref.borrow_mut() = None;
    });
    
    CAMERA.with(|camera_ref| {
        *camera_ref.borrow_mut() = None;
    });
    
    CONTROLS.with(|controls_ref| {
        *controls_ref.borrow_mut() = None;
    });
    
    EARTH_MESH.with(|earth_mesh_ref| {
        *earth_mesh_ref.borrow_mut() = None;
    });
    
    // Clear callbacks
    CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().clear();
    });
}
