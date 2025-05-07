use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use js_sys::Object;
use gloo_utils::window;
use std::rc::Rc;
use std::cell::RefCell;
use gloo::render::{AnimationFrame, request_animation_frame};
use gloo_console;
use gloo_timers::callback::Timeout;
use log;

use crate::app::components::earth::three_js::bindings::{Scene, WebGLRenderer, OrbitControls, AmbientLight, 
    MeshStandardMaterial, BufferGeometry, Material, Texture, Mesh, OrthographicCamera, WebGLRendererParams,
    IcosahedronGeometry};
use crate::app::components::earth::three_js::bindings::TextureLoader;

struct ThreeJsGlobeState {
    scene: Option<Scene>,
    camera: Option<OrthographicCamera>,
    renderer: Option<WebGLRenderer>,
    earth_mesh: Option<Mesh>,
    controls: Option<OrbitControls>,
    animation_id: Option<AnimationFrame>,
    callbacks: Vec<Closure<dyn FnMut()>>,
}

thread_local! {
    static STATE: RefCell<ThreeJsGlobeState> = RefCell::new(ThreeJsGlobeState {
        scene: None,
        camera: None,
        renderer: None,
        earth_mesh: None,
        controls: None,
        animation_id: None,
        callbacks: Vec::new(),
    });
}

fn create_earth_geometry() -> BufferGeometry {
    let earth_geometry: IcosahedronGeometry = IcosahedronGeometry::new(0.999, 50);
    let geometry_js: &JsValue = earth_geometry.as_ref();
    let buffer_geometry: BufferGeometry = geometry_js.clone().into();
    buffer_geometry
}

/// Helper function to create a material with a specific color
fn create_material_with_color(color: u32) -> MeshStandardMaterial {
    let params = js_sys::Object::new();
    js_sys::Reflect::set(&params, &"color".into(), &color.into()).unwrap();
    MeshStandardMaterial::new_with_params(&params)
}

/// Creates earth material following the original frontend implementation
fn create_earth_material(static_site: &str) -> Material {

    
    // Debug original implementation pattern
    gloo_console::log!("ORIGINAL: creating THREE.MeshStandardMaterial with map: textureLoader.load(...)");
    
    // Create a material with white base color using our helper function
    let earth_material: MeshStandardMaterial = create_material_with_color(0xFFFFFF);
    
    // Load texture
    let texture_url: String = format!("{}/earth/3_no_ice_clouds_16k.jpg", static_site);
    gloo_console::log!(format!("Loading texture from: {}", texture_url));
    
    // Create texture loader with CORS handling
    let texture_loader: TextureLoader = TextureLoader::new();
    texture_loader.set_cross_origin("anonymous");
    
    // Load the texture
    let earth_texture: Texture = texture_loader.load(&texture_url);
    
    // Check initial texture properties
    gloo_console::log!("Initial texture object:");
    gloo_console::log!(&earth_texture);
    
    // Set the texture on the material now
    gloo_console::log!("Setting texture on material");
    js_sys::Reflect::set(earth_material.as_ref(), &"map".into(), earth_texture.as_ref()).unwrap();
    
    // Create a shared reference to the material that can be used from the timer closure
    let material_ref = Rc::new(earth_material);
    let material_clone = material_ref.clone();
    
    // Set up a timer to update the material after 2 seconds when texture has loaded
    gloo_console::log!("Setting up 2-second gloo timer for texture loading");
    let timeout = Timeout::new(2_000, move || {
        gloo_console::log!("Timer complete - updating material");
        
        // Set needsUpdate to true to tell Three.js to refresh the material
        js_sys::Reflect::set(material_clone.as_ref(), &"needsUpdate".into(), &JsValue::from_bool(true)).unwrap();
        
        gloo_console::log!("Material updated with texture");
    });
    
    // Forget the timeout so it's not dropped early
    timeout.forget();
    
    // Get the material from the Rc and convert it to the right type
    // We need to explicitly convert to JsValue first
    let js_val: JsValue = wasm_bindgen::JsValue::from(material_ref.as_ref());
    let material_value = Material::from(js_val);
    
    material_value
}

/// Initialize the Three.js scene with the Earth globe
pub fn init_globe(canvas: &HtmlCanvasElement) -> Result<(), JsError> {
    gloo_console::log!("INIT GLOBE STARTING");
    cleanup_globe();
    
    let static_site = env!("CITYWARS_STATIC_SITE");
    let window = window();
    
    let diagnostic_result = super::bindings::diagnose_three_js_loading();
    gloo_console::log!("THREE diagnostics: {}", diagnostic_result);
    
    let three_available = js_sys::Reflect::has(&window, &JsValue::from_str("THREE"))
        .map_err(|_| JsError::new("Failed to check for THREE global object"))?;
    
    if !three_available {
        return Err(JsError::new("THREE global object not found. Make sure Three.js is loaded properly."));
    }
    
    js_sys::Reflect::set(
        &window,
        &JsValue::from_str("CITYWARS_STATIC_SITE"),
        &JsValue::from_str(static_site)
    ).map_err(|_| JsError::new("Failed to set static site URL"))?;
    
    let scene = Scene::new();
    gloo_console::log!("Scene created");
    
    let width = window.inner_width()
        .map_err(|_| JsError::new("Failed to get window width"))?
        .as_f64()
        .ok_or_else(|| JsError::new("Failed to convert width to f64"))?;
        
    let height = window.inner_height()
        .map_err(|_| JsError::new("Failed to get window height"))?
        .as_f64()
        .ok_or_else(|| JsError::new("Failed to convert height to f64"))?;
        
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
    
    let position = camera.position();
    position.set_z(5.0);
    gloo_console::log!("Camera created and positioned at z=5.0");
    
    let mut renderer_params = WebGLRendererParams::new();
    renderer_params.set_canvas(&canvas);
    renderer_params.set_antialias(true);
    
    let js_params: JsValue = JsValue::from(&renderer_params);
    
    let renderer: WebGLRenderer = WebGLRenderer::new_with_parameters(&js_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    
    scene.add(&AmbientLight::new(0xffffff, 2.0));
    
    let earth_mesh: Mesh = Mesh::new_with_geometry_material(
        &create_earth_geometry(), 
        &create_earth_material(static_site)
    );
    
    scene.add(&earth_mesh);
    gloo_console::log!("Earth mesh added to scene");
    
    // COMMENTED OUT: Orbit controls setup
    /*
    // Set up orbit controls similar to the original frontend
    let controls = OrbitControls::new(&camera, &renderer.domElement());
    controls.set_min_zoom(1.0);
    controls.set_max_zoom(50.0);
    controls.set_pan_speed(0.1);
    controls.set_enable_damping(true);
    controls.set_auto_rotate(true);
    controls.set_rotate_speed(1.0 / 1.5);
    
    // Store the controls in STATE before creating the closure
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.controls = Some(controls);
    });
    */
    
    // Render once without controls for now - MUST be done before moving to STATE
    renderer.render(&scene, &camera);
    
    // COMMENTED OUT: Control change callback
    /*
    // Create a callback that works with controls via STATE rather than direct capture
    let control_change_callback = Closure::wrap(Box::new(|| {
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let (Some(scene), Some(camera), Some(renderer)) = (&state.scene, &state.camera, &state.renderer) {
                renderer.render(scene, camera);
            }
        });
    }) as Box<dyn FnMut()>);
    */
    
    // Simple empty callback for now
    let control_change_callback = Closure::wrap(Box::new(|| {}) as Box<dyn FnMut()>);
    
    // Get a reference to controls from STATE to add the event listener
    STATE.with(|state_ref| {
        let state = state_ref.borrow();
        if let Some(_controls) = &state.controls {
            // controls.add_event_listener("change", &control_change_callback);
        }
    });
    
    // Store the callback so it doesn't get dropped
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.callbacks.push(control_change_callback);
    });
    
    /* COMMENTED OUT: Window resize handler
    let resize_callback = Closure::wrap(Box::new(move || {
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let (Some(renderer), Some(camera)) = (&state.renderer, &state.camera) {
                let window = web_sys::window().unwrap();
                let width = window.inner_width().unwrap().as_f64().unwrap();
                let height = window.inner_height().unwrap().as_f64().unwrap();
                let aspect = width / height;
                
                let camera_size = 1.0;
                // Update the orthographic camera parameters
                camera.set_left(-camera_size * aspect);
                camera.set_right(camera_size * aspect);
                camera.set_top(camera_size);
                camera.set_bottom(-camera_size);
                camera.update_projection_matrix();
                
                renderer.set_size(width, height);
            }
        });
    }) as Box<dyn FnMut()>);
    */
    // Simple empty callback for now
    let resize_callback = Closure::wrap(Box::new(move || {}) as Box<dyn FnMut()>);
    
    // Add the resize listener
    window.add_event_listener_with_callback("resize", resize_callback.as_ref().unchecked_ref()).unwrap();
    
    // Store the callback to prevent it from being dropped
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.callbacks.push(resize_callback);
    });
    
    // Store remaining objects in our state struct (controls already stored above)
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.scene = Some(scene);
        state.camera = Some(camera);
        state.renderer = Some(renderer);
        state.earth_mesh = Some(earth_mesh);
        // state.controls = Some(controls); // Commented out since we commented the controls
    });
    
    // COMMENTED OUT: Animation
    // start_animation()?;
    
    // No need for another render call here, already rendered above
    
    log::info!("Three.js Earth globe initialized");
    Ok(())
}

fn start_animation() -> Result<(), JsError> {
    // Create an animation loop using Rc and RefCell for shared state
    // let animation_state = Rc::new(RefCell::new(None));

    // Recursive function to request the next animation frame
    fn request_next_frame(state_ref_clone: Rc<RefCell<Option<(Scene, OrthographicCamera, WebGLRenderer)>>>){
        // Use a recursive function to create a perpetual animation loop
        let state_ref_clone = state_ref_clone.clone();
        let callback = move |_timestamp: f64| {
            // First time setup - extract the state from thread_local
            if state_ref_clone.borrow().is_none() {
                STATE.with(|global_state| {
                    let mut state = global_state.borrow_mut();
                    
                    // If we don't have the scene or renderer, return
                    if state.scene.is_none() || state.camera.is_none() || 
                       state.renderer.is_none() {
                        return;
                    }
                    
                    // Move objects from thread_local to our Rc<RefCell>
                    *state_ref_clone.borrow_mut() = Some((
                        state.scene.take().unwrap(),
                        state.camera.take().unwrap(),
                        state.renderer.take().unwrap(),
                    ));
                });
            }
            
            // Check if we have the state
            let state_exists = state_ref_clone.borrow().is_some();
            if !state_exists {
                return;
            }
            
            // Use the state to render the scene
            let mut state_borrowed = state_ref_clone.borrow_mut();
            let (scene, camera, renderer) = state_borrowed.as_mut().unwrap();
            
            // Update controls and auto-rotate the earth
            STATE.with(|state_ref| {
                let state = state_ref.borrow();
                if let Some(controls) = &state.controls {
                    controls.update();
                }
                
                // Also apply a gentle auto-rotation to the earth mesh
                if let Some(earth) = &state.earth_mesh {
                    let rotation = earth.rotation();
                    rotation.set_y(rotation.y() + 0.002);
                }
            });
            
            // Render the scene
            let camera_js: &JsValue = camera.as_ref();
            renderer.render(scene, camera_js);
            
            // Log the first few animation frames to confirm rendering is happening
            static mut FRAME_COUNT: u32 = 0;
            unsafe {
                if FRAME_COUNT < 5 {
                    // Fix the formatting syntax for gloo_console
                    let frame_msg = format!("Animation frame rendered: {}", FRAME_COUNT);
                    gloo_console::log!(frame_msg);
                    FRAME_COUNT += 1;
                }
            }
            
            // Schedule the next animation frame
            request_next_frame(state_ref_clone.clone());
        };
        
        // Request a single animation frame
        let handle = request_animation_frame(callback);
        
        // Store the animation frame
        STATE.with(|state_ref| {
            let mut state = state_ref.borrow_mut();
            state.animation_id = Some(handle);
        });
    }
    
    // Start the animation loop
    let shared_state = Rc::new(RefCell::new(None));
    request_next_frame(shared_state);
    
    Ok(())
}

/// Clean up Three.js resources
pub fn cleanup_globe() {
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        
        // Dispose of renderer if needed
        if let Some(renderer) = &state.renderer {
            renderer.dispose();
        }
        
        // Reset all state - AnimationFrame automatically cancels on drop
        state.animation_id = None;
        state.scene = None;
        state.camera = None;
        state.renderer = None;
        state.earth_mesh = None;
        state.controls = None;
        state.callbacks.clear();
    });

}
