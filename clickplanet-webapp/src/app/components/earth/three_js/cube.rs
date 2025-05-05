use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use std::cell::RefCell;
use std::rc::Rc;
use gloo::render::{AnimationFrame, request_animation_frame};
use gloo_utils::window;

use super::bindings::{WebGLRendererParams, WebGLRenderer, MeshBasicMaterialParams};
use super::bindings::*;

struct ThreeJsState {
    scene: Option<Scene>,
    camera: Option<PerspectiveCamera>,
    renderer: Option<WebGLRenderer>,
    cube: Option<Mesh>,
    animation_frame: Option<AnimationFrame>,
}

thread_local! {
    static STATE: RefCell<ThreeJsState> = RefCell::new(ThreeJsState {
        scene: None,
        camera: None,
        renderer: None,
        cube: None,
        animation_frame: None,
    });
}

pub fn init_cube(canvas: &HtmlCanvasElement) -> Result<(), JsError> {
    cleanup_cube();
    
    let scene = Scene::new();

    let window = window();
    

    let width = window.inner_width()
        .map_err(|_| JsError::new("Failed to get window width"))?
        .as_f64()
        .ok_or_else(|| JsError::new("Failed to convert width to f64"))?;
        
    let height = window.inner_height()
        .map_err(|_| JsError::new("Failed to get window height"))?
        .as_f64()
        .ok_or_else(|| JsError::new("Failed to convert height to f64"))?;
        
    let aspect = width / height;
    
    let camera = PerspectiveCamera::new(75.0, aspect, 0.1, 1000.0);
    camera.position().set_z(5.0);
    

    let mut renderer_params = WebGLRendererParams::new();
    renderer_params.set_canvas(canvas); 
    renderer_params.set_antialias(true);
    renderer_params.set_alpha(true); 
    
    let js_params = JsValue::from(&renderer_params);
    let renderer = WebGLRenderer::new_with_parameters(&js_params);
    renderer.set_size(width, height);
    renderer.set_clear_color(0x000000);
    
    let geometry = BoxGeometry::new(1.0, 1.0, 1.0);
    
    let mut material_params = MeshBasicMaterialParams::new();
    material_params.set_color(0x00ff00); 
    
    let js_material_params = JsValue::from(&material_params);    
    let material = MeshBasicMaterial::new_with_params(&js_material_params);
    

    let geometry_js: &JsValue = geometry.as_ref();
    let material_js: &JsValue = material.as_ref();
    

    let buffer_geometry = geometry_js.clone().into();
    let material_value = material_js.clone().into();
    

    let cube = Mesh::new_with_geometry_material(
        &buffer_geometry, 
        &material_value,
    );
    

    scene.add(&cube);
    

    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        state.scene = Some(scene);
        state.camera = Some(camera);
        state.renderer = Some(renderer);
        state.cube = Some(cube);
    });
    


    start_animation()?;


    Ok(())
}

fn start_animation() -> Result<(), JsError> {

    fn request_next_frame(state_ref: Rc<RefCell<Option<(Scene, PerspectiveCamera, WebGLRenderer, Mesh)>>>) {


        let state_ref_clone = state_ref.clone();
        let callback = move |_timestamp: f64| {

    
            if state_ref_clone.borrow().is_none() {
                STATE.with(|global_state| {
                    let mut state = global_state.borrow_mut();
                    
            
                    if state.scene.is_none() || state.camera.is_none() || 
                       state.renderer.is_none() || state.cube.is_none() {
                        return;
                    }
                    
            
                    *state_ref_clone.borrow_mut() = Some((
                        state.scene.take().unwrap(),
                        state.camera.take().unwrap(),
                        state.renderer.take().unwrap(),
                        state.cube.take().unwrap(),
                    ));
                });
            }
            
    
            let state_exists = state_ref_clone.borrow().is_some();
            if !state_exists {
                return;
            }
            
    
            let mut state_borrowed = state_ref_clone.borrow_mut();
            let (scene, camera, renderer, cube) = state_borrowed.as_mut().unwrap();
            
    
            let current_x = cube.rotation().x();
            let current_y = cube.rotation().y();
            cube.rotation().set_x(current_x + 0.01);
            cube.rotation().set_y(current_y + 0.01);
            
    
            let camera_js: &JsValue = camera.as_ref();
            renderer.render(scene, camera_js);
            
    
            request_next_frame(state_ref_clone.clone());
        };
        

        let handle = request_animation_frame(callback);
        

        STATE.with(|state_ref| {
            let mut state = state_ref.borrow_mut();
            state.animation_frame = Some(handle);
        });
    }
    

    let shared_state = Rc::new(RefCell::new(None));
    request_next_frame(shared_state);
    
    Ok(())
}

pub fn cleanup_cube() {

    STATE.with(|state_ref| {
        let mut state = state_ref.borrow_mut();
        
    
        if let Some(renderer) = &state.renderer {
            renderer.dispose();
        }
        
    
        state.animation_frame = None;
        state.scene = None;
        state.camera = None;
        state.renderer = None;
        state.cube = None;
    });
}
