use wasm_bindgen::prelude::*;
use web_sys::Element;
use js_sys;

#[wasm_bindgen]
pub struct WebGLRendererParams {
    #[wasm_bindgen(skip)]
    pub canvas: Option<web_sys::HtmlCanvasElement>,
    
    pub alpha: Option<bool>,
    pub antialias: Option<bool>,
    pub depth: Option<bool>,
    pub premultiplied_alpha: Option<bool>,
    pub preserve_drawing_buffer: Option<bool>,
}

#[wasm_bindgen]
impl WebGLRendererParams {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            canvas: None,
            alpha: None,
            antialias: None,
            depth: None,
            premultiplied_alpha: None,
            preserve_drawing_buffer: None,
        }
    }
    
    pub fn set_canvas(&mut self, canvas: &web_sys::HtmlCanvasElement) {
        self.canvas = Some(canvas.clone());
    }
    
    pub fn set_alpha(&mut self, alpha: bool) {
        self.alpha = Some(alpha);
    }
    
    pub fn set_antialias(&mut self, antialias: bool) {
        self.antialias = Some(antialias);
    }
    
    pub fn set_depth(&mut self, depth: bool) {
        self.depth = Some(depth);
    }
    
    pub fn set_premultiplied_alpha(&mut self, premultiplied_alpha: bool) {
        self.premultiplied_alpha = Some(premultiplied_alpha);
    }
    
    pub fn set_preserve_drawing_buffer(&mut self, preserve_drawing_buffer: bool) {
        self.preserve_drawing_buffer = Some(preserve_drawing_buffer);
    }
}

impl From<&WebGLRendererParams> for JsValue {
    fn from(params: &WebGLRendererParams) -> Self {
        let obj = js_sys::Object::new();
        
        if let Some(canvas) = &params.canvas {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("canvas"), canvas);
        }
        
        if let Some(alpha) = params.alpha {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("alpha"), &JsValue::from_bool(alpha));
        }
        
        if let Some(antialias) = params.antialias {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("antialias"), &JsValue::from_bool(antialias));
        }
        
        if let Some(depth) = params.depth {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("depth"), &JsValue::from_bool(depth));
        }
        
        if let Some(premultiplied_alpha) = params.premultiplied_alpha {
            let _ = js_sys::Reflect::set(
                &obj, 
                &JsValue::from_str("premultipliedAlpha"), 
                &JsValue::from_bool(premultiplied_alpha)
            );
        }
        
        if let Some(preserve_drawing_buffer) = params.preserve_drawing_buffer {
            let _ = js_sys::Reflect::set(
                &obj, 
                &JsValue::from_str("preserveDrawingBuffer"), 
                &JsValue::from_bool(preserve_drawing_buffer)
            );
        }
        
        obj.into()
    }
}

#[wasm_bindgen]
pub struct MeshBasicMaterialParams {
    pub color: Option<u32>,
    pub wireframe: Option<bool>,
    pub transparent: Option<bool>,
    pub opacity: Option<f64>,
}

#[wasm_bindgen]
impl MeshBasicMaterialParams {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            color: None,
            wireframe: None,
            transparent: None,
            opacity: None,
        }
    }
    
    pub fn set_color(&mut self, color: u32) {
        self.color = Some(color);
    }
    
    pub fn set_wireframe(&mut self, wireframe: bool) {
        self.wireframe = Some(wireframe);
    }
    
    pub fn set_transparent(&mut self, transparent: bool) {
        self.transparent = Some(transparent);
    }
    
    pub fn set_opacity(&mut self, opacity: f64) {
        self.opacity = Some(opacity);
    }
}

impl From<&MeshBasicMaterialParams> for JsValue {
    fn from(params: &MeshBasicMaterialParams) -> Self {
        let obj = js_sys::Object::new();
        
        if let Some(color) = params.color {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("color"), &JsValue::from_f64(color as f64));
        }
        
        if let Some(wireframe) = params.wireframe {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("wireframe"), &JsValue::from_bool(wireframe));
        }
        
        if let Some(transparent) = params.transparent {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("transparent"), &JsValue::from_bool(transparent));
        }
        
        if let Some(opacity) = params.opacity {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("opacity"), &JsValue::from_f64(opacity));
        }
        
        obj.into()
    }
}

#[wasm_bindgen]
extern "C" {
    // Main Three.js namespace
    #[wasm_bindgen(js_namespace = THREE)]
    pub type Scene;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new() -> Scene;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = add)]
    pub fn add(this: &Scene, object: &Object3D);

    #[wasm_bindgen(js_namespace = THREE)]
    pub type Object3D;
    
    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn rotation(this: &Object3D) -> Euler;

    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type Mesh;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_geometry_material(geometry: &BufferGeometry, material: &Material) -> Mesh;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type WebGLRenderer;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new() -> WebGLRenderer;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_parameters(params: &JsValue) -> WebGLRenderer;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = setSize)]
    pub fn set_size(this: &WebGLRenderer, width: f64, height: f64);

    #[wasm_bindgen(method, js_namespace = THREE, js_name = setClearColor)]
    pub fn set_clear_color(this: &WebGLRenderer, color: u32);

    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn domElement(this: &WebGLRenderer) -> Element;

    // Generic render method that can handle any camera type by using JsValue
    #[wasm_bindgen(method, js_namespace = THREE, js_name = render)]
    pub fn render(this: &WebGLRenderer, scene: &Scene, camera: &JsValue);

    #[wasm_bindgen(method, js_namespace = THREE, js_name = dispose)]
    pub fn dispose(this: &WebGLRenderer);
    
    // PerspectiveCamera for the cube example
    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type PerspectiveCamera;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(fov: f64, aspect: f64, near: f64, far: f64) -> PerspectiveCamera;
    
    #[wasm_bindgen(method, getter, js_namespace = THREE, js_name = position)]
    pub fn position(this: &PerspectiveCamera) -> Vector3;

    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type OrthographicCamera;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(left: f64, right: f64, top: f64, bottom: f64, near: f64, far: f64) -> OrthographicCamera;

    #[wasm_bindgen(method, getter, js_namespace = THREE, js_name = position)]
    pub fn position(this: &OrthographicCamera) -> Vector3;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = updateProjectionMatrix)]
    pub fn update_projection_matrix(this: &OrthographicCamera);

    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn zoom(this: &OrthographicCamera) -> f64;

    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_left(this: &OrthographicCamera, left: f64);

    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_right(this: &OrthographicCamera, right: f64);

    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_top(this: &OrthographicCamera, top: f64);

    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_bottom(this: &OrthographicCamera, bottom: f64);

    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type AmbientLight;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(color: u32, intensity: f64) -> AmbientLight;

    // Base BufferGeometry type
    #[wasm_bindgen(js_namespace = THREE)]
    pub type BufferGeometry;
    
    // BoxGeometry extends BufferGeometry
    #[wasm_bindgen(js_namespace = THREE)]
    pub type BoxGeometry;
    
    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(width: f64, height: f64, depth: f64) -> BoxGeometry;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type IcosahedronGeometry;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(radius: f64, detail: u32) -> IcosahedronGeometry;
    
    // Base Material type
    #[wasm_bindgen(js_namespace = THREE)]
    pub type Material;
    
    // MeshBasicMaterial extends Material
    #[wasm_bindgen(js_namespace = THREE)]
    pub type MeshBasicMaterial;
    
    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_params(params: &JsValue) -> MeshBasicMaterial;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type MeshStandardMaterial;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_params(params: &JsValue) -> MeshStandardMaterial;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type Texture;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type TextureLoader;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new() -> TextureLoader;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = load)]
    pub fn load(this: &TextureLoader, url: &str) -> Texture;
    
    // Euler for rotation
    #[wasm_bindgen(js_namespace = THREE)]
    pub type Euler;
    
    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn x(this: &Euler) -> f64;
    
    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn y(this: &Euler) -> f64;
    
    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_x(this: &Euler, x: f64);
    
    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_y(this: &Euler, y: f64);

    #[wasm_bindgen(js_namespace = THREE)]
    pub type Vector3;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(x: f64, y: f64, z: f64) -> Vector3;

    #[wasm_bindgen(method, setter, js_namespace = THREE)]
    pub fn set_z(this: &Vector3, z: f64);

    #[wasm_bindgen(js_namespace = THREE)]
    pub type Vector2;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(x: f64, y: f64) -> Vector2;

}

// Function to check if Three.js is available in the global scope
pub fn is_three_available() -> bool {
    use wasm_bindgen::prelude::*;
    use web_sys::console;
    
    // Try to access the THREE global object via window
    console::log_1(&JsValue::from_str("Checking for THREE global object..."));
    
    if let Some(window) = web_sys::window() {
        // Log all available global objects for debugging
        if let Ok(keys) = js_sys::Reflect::own_keys(&window) {
            console::log_2(
                &JsValue::from_str("Window global objects available:"),
                &keys
            );
        }
        
        // Check if THREE is directly available on window
        if js_sys::Reflect::has(&window, &JsValue::from_str("THREE")).unwrap_or(false) {
            console::log_1(&JsValue::from_str("THREE found directly on window object!"));
            return true;
        }
        
        // Try the eval approach as a backup
        let eval_script = r#"(function() { 
            try { 
                if (typeof THREE !== 'undefined') {
                    console.log('THREE found with type:', typeof THREE);
                    console.log('THREE version:', THREE.REVISION || 'unknown');
                    return true; 
                } else {
                    console.log('THREE is undefined in global scope');
                    return false;
                }
            } catch(err) {
                console.error('Error checking THREE:', err);
                return false;
            }
        })()
        "#;
        
        // Execute the evaluation script
        if let Ok(result) = js_sys::eval(eval_script) {
            console::log_1(&JsValue::from_str(&format!("THREE availability check result: {}", result.is_truthy())));
            return result.is_truthy();
        }
    }
    
    console::log_1(&JsValue::from_str("Failed to check THREE availability"));
    false
}

// OrbitControls bindings - using standard imports
#[wasm_bindgen]
extern "C" {
    pub type OrbitControls;

    #[wasm_bindgen(constructor)]
    pub fn new(camera: &OrthographicCamera, dom_element: &Element) -> OrbitControls;

    #[wasm_bindgen(method)]
    pub fn update(this: &OrbitControls);

    #[wasm_bindgen(method, setter)]
    pub fn set_min_zoom(this: &OrbitControls, min_zoom: f64);

    #[wasm_bindgen(method, setter)]
    pub fn set_max_zoom(this: &OrbitControls, max_zoom: f64);

    #[wasm_bindgen(method, setter)]
    pub fn set_pan_speed(this: &OrbitControls, speed: f64);

    #[wasm_bindgen(method, setter)]
    pub fn set_enable_damping(this: &OrbitControls, enable: bool);

    #[wasm_bindgen(method, setter)]
    pub fn set_auto_rotate(this: &OrbitControls, auto_rotate: bool);

    #[wasm_bindgen(method, setter)]
    pub fn set_auto_rotate_speed(this: &OrbitControls, speed: f64);

    #[wasm_bindgen(method, setter)]
    pub fn set_rotate_speed(this: &OrbitControls, speed: f64);

    #[wasm_bindgen(method, js_name = addEventListener)]
    pub fn add_event_listener(this: &OrbitControls, event_name: &str, callback: &Closure<dyn FnMut()>);
}
