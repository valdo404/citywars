use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use gloo_utils::window;
use std::cell::RefCell;
use gloo_render::AnimationFrame;
use gloo_console;
use log;

use crate::app::components::earth::three_js::bindings::*;

struct ThreeJsGlobeState {
    scene: Option<Scene>,
    camera: Option<OrthographicCamera>,
    renderer: Option<WebGLRenderer>,
    earth_mesh: Option<Mesh>,
    earth_material: Option<MeshStandardMaterial>,
    controls: Option<OrbitControls>,
    animation_id: Option<AnimationFrame>,
    resize_callback: Option<Closure<dyn FnMut()>>,
    animation_callback: Option<Closure<dyn FnMut(f64)>>,
    texture_callback: Option<Closure<dyn FnMut(Texture)>>,
    error_callback: Option<Closure<dyn FnMut(JsValue)>>,
}

thread_local! {
    static STATE: RefCell<ThreeJsGlobeState> = RefCell::new(ThreeJsGlobeState {
        scene: None,
        camera: None,
        renderer: None,
        earth_mesh: None,
        controls: None,
        animation_id: None,
        resize_callback: None,
        animation_callback: None,
        earth_material: None,
        texture_callback: None,
        error_callback: None,
    });
}

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
    
    let mut renderer_params = WebGLRendererParams::new(&canvas);
    renderer_params.set_antialias(true);
    
     let js_params = serde_wasm_bindgen::to_value(&renderer_params).unwrap();

    let renderer: WebGLRenderer = WebGLRenderer::new_with_parameters(&js_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    scene.add(&AmbientLight::new(0xffffff, 2.0));
        
    let earth_geometry = IcosahedronGeometry::new(0.999, 50);
    
    let material_params = js_sys::Object::new();
    js_sys::Reflect::set(&material_params, &JsValue::from_str("color"), &JsValue::from_f64(0xFFFFFF as f64))
        .expect("Failed to set material color");
    
    let earth_material = MeshStandardMaterial::new_with_params(&material_params);
    
    let earth_geometry_js: &JsValue = earth_geometry.as_ref();
    let earth_geometry_as_buffer: &BufferGeometry = earth_geometry_js.unchecked_ref();
    
    let earth_material_js: &JsValue = earth_material.as_ref();
    let earth_material_as_material: &Material = earth_material_js.unchecked_ref();
    
    let earth_mesh = Mesh::new_with_geometry_material(earth_geometry_as_buffer, earth_material_as_material);
    
    scene.add(&earth_mesh);
    
    let texture_loader = TextureLoader::new();
    texture_loader.set_cross_origin("anonymous");
    let texture_url = format!("{}/earth/3_no_ice_clouds_16k.jpg", static_site);
    
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.earth_material = Some(earth_material);
    });
    
    let on_load_callback = Closure::wrap(Box::new(move |texture: Texture| {        
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let Some(material) = &state.earth_material {
                js_sys::Reflect::set(material.as_ref(), &JsValue::from_str("map"), &texture)
                    .expect("Failed to set texture map");
                
                js_sys::Reflect::set(material.as_ref(), &JsValue::from_str("needsUpdate"), &JsValue::from_bool(true))
                    .expect("Failed to set needsUpdate");
            }
        });
    }) as Box<dyn FnMut(Texture)>);
    
    let on_error_callback = Closure::wrap(Box::new(move |err: JsValue| {
        gloo_console::error!("Error loading texture: {:?}", err);
    }) as Box<dyn FnMut(JsValue)>);

    let resize_callback = Closure::wrap(Box::new(move || {
        let window = web_sys::window().unwrap();
        let width = window.inner_width().unwrap().as_f64().unwrap();
        let height = window.inner_height().unwrap().as_f64().unwrap();
        let aspect = width / height;
        
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let (Some(renderer), Some(camera)) = (&state.renderer, &state.camera) {
                let camera_size = 1.0;
                camera.set_left(-camera_size * aspect);
                camera.set_right(camera_size * aspect);
                camera.set_top(camera_size);
                camera.set_bottom(-camera_size);
                camera.update_projection_matrix();
                
                renderer.set_size(width, height);
                
                if let Some(scene) = &state.scene {
                    renderer.render(&scene, &camera);
                }
                
                gloo_console::log!("Window resized: {}x{}", width, height);
            }
        });
    }) as Box<dyn FnMut()>);
    
    let animate_callback = Closure::wrap(Box::new(|_timestamp: f64| {
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let (Some(scene), Some(camera), Some(renderer)) = (&state.scene, &state.camera, &state.renderer) {
                renderer.render(scene, camera);
            }
        });
        
        if let Some(window) = web_sys::window() {
            STATE.with(|state_ref| {
                let state = state_ref.borrow();
                if let Some(callback) = &state.animation_callback {
                    let callback_js_ref = callback.as_ref().unchecked_ref();
                    let _ = window.request_animation_frame(callback_js_ref);
                }
            });
        }
    }) as Box<dyn FnMut(f64)>);
    

    let on_load_js_ref = on_load_callback.as_ref().unchecked_ref();
    let on_error_js_ref = on_error_callback.as_ref().unchecked_ref();

    texture_loader.load_with_callbacks(
        &texture_url,
        on_load_js_ref,
        &JsValue::UNDEFINED, 
        on_error_js_ref
    );

    let resize_js_ref = resize_callback.as_ref().unchecked_ref();
    window.add_event_listener_with_callback("resize", resize_js_ref)
        .map_err(|_| JsError::new("Failed to add resize event listener"))?;

    if let Some(window) = web_sys::window() {
        let callback_js_ref = animate_callback.as_ref().unchecked_ref();
        let _ = window.request_animation_frame(callback_js_ref);
    }

    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.texture_callback = Some(on_load_callback);
        state.error_callback = Some(on_error_callback);
        state.resize_callback = Some(resize_callback);
        state.animation_callback = Some(animate_callback);
    });

    // Make iitial render
    renderer.render(&scene, &camera);
    
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.scene = Some(scene);
        state.camera = Some(camera);
        state.renderer = Some(renderer);
        state.earth_mesh = Some(earth_mesh);
        // state.controls = None; // Orbit controls are disabled
    });


    /*
    // Set up orbit controls similar to the original frontend
    let controls = OrbitControls::new(&camera, &renderer.domElement());
    controls.set_min_zoom(1.0);
    controls.set_max_zoom(50.0);
    controls.set_pan_speed(0.1);
    controls.set_zoom_speed(0.5);
    controls.set_enable_damping(true);
    controls.set_auto_rotate(true);
    controls.set_rotate_speed(1.0 / 1.5);
    */
    
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
    //let control_change_callback = Closure::wrap(Box::new(|| {}) as Box<dyn FnMut()>);
    
    // Get a reference to controls from STATE to add the event listener
    /*STATE.with(|state_ref| {
        let state = state_ref.borrow();
        if let Some(_controls) = &state.controls {
            // controls.add_event_listener("change", &control_change_callback);
        }
    });*/
    
    // Store the callback so it doesn't get dropped
    /*STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.callbacks.push(control_change_callback);
    });*/
    
    // No need for another render call here, already rendered above
    
    log::info!("Three.js Earth globe initialized");
    Ok(())
}

/// Clean up Three.js resources
pub fn cleanup_globe() {
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        
        if let Some(renderer) = &state.renderer {
            renderer.dispose();
        }
        
        state.animation_id = None;
        state.scene = None;
        state.camera = None;
        state.renderer = None;
        state.earth_mesh = None;
        state.controls = None;
        state.resize_callback = None;
        state.animation_callback = None;
        state.texture_callback = None;
        state.error_callback = None;
        state.earth_material = None;
    });

}
