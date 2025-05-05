use wasm_bindgen::prelude::*;
use web_sys::Element;

// Three.js bindings - using standard imports without module paths
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

    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type Mesh;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_geometry_material(geometry: &BufferGeometry, material: &Material) -> Mesh;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type WebGLRenderer;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new_with_parameters(params: &JsValue) -> WebGLRenderer;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = setSize)]
    pub fn set_size(this: &WebGLRenderer, width: f64, height: f64);

    #[wasm_bindgen(method, js_namespace = THREE, js_name = setClearColor)]
    pub fn set_clear_color(this: &WebGLRenderer, color: u32);

    #[wasm_bindgen(method, getter, js_namespace = THREE)]
    pub fn domElement(this: &WebGLRenderer) -> Element;

    #[wasm_bindgen(method, js_namespace = THREE, js_name = render)]
    pub fn render(this: &WebGLRenderer, scene: &Scene, camera: &OrthographicCamera);

    #[wasm_bindgen(method, js_namespace = THREE, js_name = dispose)]
    pub fn dispose(this: &WebGLRenderer);

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

    #[wasm_bindgen(extends = Object3D, js_namespace = THREE)]
    pub type AmbientLight;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(color: u32, intensity: f64) -> AmbientLight;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type BufferGeometry;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type IcosahedronGeometry;

    #[wasm_bindgen(constructor, js_namespace = THREE)]
    pub fn new(radius: f64, detail: u32) -> IcosahedronGeometry;

    #[wasm_bindgen(js_namespace = THREE)]
    pub type Material;

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
