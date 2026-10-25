//! Orthographic orbit camera that reproduces the behaviour of the original front end, which used
//! three.js' `OrthographicCamera` (half height 1, camera at distance 5) driven by `OrbitControls`
//! with `minZoom = 1`, `maxZoom = 50`, `enableDamping = true` (factor 0.05), `autoRotate` while the
//! zoom is 1 (speed 2, i.e. one turn every 30 s) and `rotateSpeed = (1 / zoom) / 1.5`.

use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::PI;

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 50.0;
const DAMPING: f32 = 0.05;
const CAMERA_DISTANCE: f32 = 5.0;
/// three.js `OrbitControls.autoRotateSpeed` default (2.0 = 30 s per turn).
const AUTO_ROTATE_SPEED: f32 = 2.0;
const POLAR_EPS: f32 = 1e-6;
/// three.js `getZoomScale` base.
const ZOOM_BASE: f32 = 0.95;

#[derive(Debug, Clone)]
pub struct OrbitControls {
    /// Azimuth around the Y axis (three.js `Spherical.theta`).
    pub theta: f32,
    /// Polar angle from +Y (three.js `Spherical.phi`).
    pub phi: f32,
    pub zoom: f32,
    /// Auto-rotation is allowed (it only happens while the zoom is 1).
    pub auto_rotate_enabled: bool,
    delta_theta: f32,
    delta_phi: f32,
}

impl Default for OrbitControls {
    fn default() -> Self {
        // camera at (0, 0, 5) like the original
        OrbitControls {
            theta: 0.0,
            phi: PI / 2.0,
            zoom: MIN_ZOOM,
            auto_rotate_enabled: true,
            delta_theta: 0.0,
            delta_phi: 0.0,
        }
    }
}

/// Camera matrices for one frame.
#[derive(Debug, Clone, Copy)]
pub struct CameraFrame {
    pub eye: Vec3,
    pub view_proj: Mat4,
    /// World-space camera right and up vectors (used to billboard the tiles).
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub half_width: f32,
    pub half_height: f32,
}

impl OrbitControls {
    pub fn auto_rotate(&self) -> bool {
        self.auto_rotate_enabled && self.zoom <= MIN_ZOOM
    }

    pub fn rotate_speed(&self) -> f32 {
        (1.0 / self.zoom) / 1.5
    }

    /// Drag by `delta` CSS pixels on an element `element_height` pixels high.
    pub fn rotate_by_pixels(&mut self, delta: Vec2, element_height: f32) {
        let h = element_height.max(1.0);
        let speed = self.rotate_speed();
        self.delta_theta -= 2.0 * PI * delta.x / h * speed;
        self.delta_phi -= 2.0 * PI * delta.y / h * speed;
    }

    /// Mouse wheel, `delta_y` in pixels (positive = scroll down = zoom out), like three.js.
    pub fn wheel(&mut self, delta_y: f32) {
        if delta_y == 0.0 {
            return;
        }
        let scale = ZOOM_BASE.powf((delta_y * 0.01).abs());
        if delta_y < 0.0 {
            self.set_zoom(self.zoom / scale);
        } else {
            self.set_zoom(self.zoom * scale);
        }
    }

    /// Pinch: the distance between the two fingers went from `previous` to `current`.
    pub fn pinch(&mut self, previous: f32, current: f32) {
        if previous > 0.0 && current > 0.0 {
            self.set_zoom(self.zoom * current / previous);
        }
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Advances damping and auto-rotation by one frame (`dt` seconds).
    pub fn update(&mut self, dt: f32) {
        if self.auto_rotate() {
            self.delta_theta -= 2.0 * PI / 60.0 * AUTO_ROTATE_SPEED * dt;
        }
        self.theta += self.delta_theta * DAMPING;
        self.phi = (self.phi + self.delta_phi * DAMPING).clamp(POLAR_EPS, PI - POLAR_EPS);
        self.delta_theta *= 1.0 - DAMPING;
        self.delta_phi *= 1.0 - DAMPING;
    }

    pub fn frame(&self, aspect: f32) -> CameraFrame {
        let eye = Vec3::new(
            CAMERA_DISTANCE * self.phi.sin() * self.theta.sin(),
            CAMERA_DISTANCE * self.phi.cos(),
            CAMERA_DISTANCE * self.phi.sin() * self.theta.cos(),
        );
        let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        let half_height = 1.0 / self.zoom;
        let half_width = aspect / self.zoom;
        // glam's orthographic_rh maps depth to [0, 1], as wgpu expects
        let proj = Mat4::orthographic_rh(-half_width, half_width, -half_height, half_height, 0.01, 100.0);
        let right = view.row(0).truncate();
        let up = view.row(1).truncate();
        CameraFrame {
            eye,
            view_proj: proj * view,
            right,
            up,
            forward: -view.row(2).truncate(),
            half_width,
            half_height,
        }
    }
}

impl CameraFrame {
    /// Intersects the ray under a screen position (`ndc` in [-1, 1], y up) with the unit sphere and
    /// returns the visible hit point.
    pub fn pick_sphere(&self, ndc: Vec2) -> Option<Vec3> {
        let origin = self.eye + self.right * ndc.x * self.half_width + self.up * ndc.y * self.half_height;
        let dir = self.forward;
        // |origin + t dir|^2 = 1
        let b = origin.dot(dir);
        let c = origin.length_squared() - 1.0;
        let disc = b * b - c;
        if disc < 0.0 {
            return None;
        }
        let t = -b - disc.sqrt();
        (t > 0.0).then(|| origin + dir * t)
    }

    /// World units per CSS pixel for a viewport `height` pixels high.
    pub fn world_per_pixel(&self, height: f32) -> f32 {
        2.0 * self.half_height / height.max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_like_the_original_camera() {
        let c = OrbitControls::default();
        let f = c.frame(1.0);
        assert!((f.eye - Vec3::new(0.0, 0.0, 5.0)).length() < 1e-5);
        assert!(c.auto_rotate());
        // the centre of the screen hits the point of the sphere facing the camera
        let hit = f.pick_sphere(Vec2::ZERO).unwrap();
        assert!((hit - Vec3::Z).length() < 1e-5);
        // outside the globe there is no hit
        assert!(f.pick_sphere(Vec2::new(0.99, 0.99)).is_none());
    }

    #[test]
    fn zoom_is_clamped_and_stops_auto_rotation() {
        let mut c = OrbitControls::default();
        c.wheel(100.0);
        assert_eq!(c.zoom, MIN_ZOOM);
        c.wheel(-100.0);
        assert!(c.zoom > MIN_ZOOM);
        assert!(!c.auto_rotate());
        c.pinch(1.0, 1000.0);
        assert_eq!(c.zoom, MAX_ZOOM);
    }

    #[test]
    fn auto_rotation_turns_once_every_30_seconds() {
        let mut c = OrbitControls::default();
        for _ in 0..(60 * 30) {
            c.update(1.0 / 60.0);
        }
        // damping lags slightly behind, but the camera has done about one full turn
        let turns = -c.theta / (2.0 * PI);
        assert!((turns - 1.0).abs() < 0.02, "turns = {turns}");
    }

    #[test]
    fn drag_has_inertia_and_polar_angle_is_clamped() {
        let mut c = OrbitControls::default();
        c.set_zoom(2.0);
        c.rotate_by_pixels(Vec2::new(0.0, -10_000.0), 500.0);
        c.update(1.0 / 60.0);
        let after_one = c.phi;
        for _ in 0..200 {
            c.update(1.0 / 60.0);
        }
        assert!(c.phi >= after_one);
        assert!(c.phi <= PI);
    }
}
