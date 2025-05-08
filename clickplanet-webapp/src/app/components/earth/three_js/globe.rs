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
    control_callback: Option<Closure<dyn FnMut()>>,
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
        control_callback: None
    });
}

pub fn init_globe(canvas: &HtmlCanvasElement) -> Result<(), JsError> {
    gloo_console::log!("INIT GLOBE STARTING");
    cleanup_globe();
    
    let static_site = env!("CITYWARS_STATIC_SITE");
    let window = window();
    
    let scene: Scene = Scene::new();
    
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
    
    let camera: OrthographicCamera = OrthographicCamera::new(
        -camera_size * aspect,
        camera_size * aspect,
        camera_size,
        -camera_size,
        0.01,
        100.0
    );
    
    let position = camera.position();
    position.set_z(5.0);
    
    let mut renderer_params: WebGLRendererParams = WebGLRendererParams::new(&canvas);
    renderer_params.set_antialias(true);

    let renderer: WebGLRenderer = WebGLRenderer::new_with_parameters(renderer_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    scene.add(&AmbientLight::new(0xffffff, 2.0));
        
    let earth_geometry: IcosahedronGeometry = IcosahedronGeometry::new(0.999, 50);
    
    let mut material_params = MeshStandardMaterialParams::new();
    material_params.set_color(0xFFFFFF);
    
    let earth_material: MeshStandardMaterial = MeshStandardMaterial::new_with_params(material_params);
    
    let earth_mesh = Mesh::new_with_geometry_material(earth_geometry.as_ref(), earth_material.as_ref());
    
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
                material.set_map(&texture);
                material.set_needs_update(true);
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

            if let Some(controls) = &state.controls {
                controls.update();
            }
            
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
    
    let control_change_callback = Closure::wrap(Box::new(|| {
        STATE.with(|state_ref| {
            let state = state_ref.borrow();
            if let (Some(camera), Some(controls)) = (&state.camera, &state.controls) {
                let zoom = camera.zoom();
                let should_auto_rotate = zoom == 1.0;
                controls.set_auto_rotate(should_auto_rotate);
                
                let rotate_speed = (1.0 / zoom) / 1.5;
                controls.set_rotate_speed(rotate_speed);
            } else {
                gloo_console::warn!("Control change called but camera or controls are None");
                if state.camera.is_none() {
                    gloo_console::warn!("Camera is None");
                }
                if state.controls.is_none() {
                    gloo_console::warn!("Controls is None");
                }
            }
            
            if let (Some(scene), Some(camera), Some(renderer)) = (&state.scene, &state.camera, &state.renderer) {
                renderer.render(scene, camera);
            }
        });
    }) as Box<dyn FnMut()>);

    let on_load_js_ref = on_load_callback.as_ref().unchecked_ref();
    let on_error_js_ref = on_error_callback.as_ref().unchecked_ref();

    texture_loader.load_with_callbacks(
        &texture_url,
        on_load_js_ref,
        &JsValue::UNDEFINED, 
        on_error_js_ref
    );

    let controls = OrbitControls::new(&camera, &renderer.dom_element());
    controls.set_min_zoom(1.0);
    controls.set_max_zoom(50.0);
    controls.set_pan_speed(0.1);
    controls.set_enable_damping(true);
    controls.set_auto_rotate(true);
    controls.set_rotate_speed(1.0 / 1.5);

    let resize_js_ref = resize_callback.as_ref().unchecked_ref();
    window.add_event_listener_with_callback("resize", resize_js_ref)
        .map_err(|_| JsError::new("Failed to add resize event listener"))?;


    if let Some(window) = web_sys::window() {
        let callback_js_ref = animate_callback.as_ref().unchecked_ref();
        let _ = window.request_animation_frame(callback_js_ref);
    }

    controls.add_event_listener("change", &control_change_callback);
    
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.controls = Some(controls);
    });

    renderer.render(&scene, &camera);
    
    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.texture_callback = Some(on_load_callback);
        state.error_callback = Some(on_error_callback);
        state.resize_callback = Some(resize_callback);
        state.animation_callback = Some(animate_callback);
        state.control_callback = Some(control_change_callback);
        state.scene = Some(scene);
        state.camera = Some(camera);
        state.renderer = Some(renderer);
        state.earth_mesh = Some(earth_mesh);
    });
            
    log::info!("Three.js Earth globe initialized");
    Ok(())
}

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
        state.control_callback = None;
    });

}
