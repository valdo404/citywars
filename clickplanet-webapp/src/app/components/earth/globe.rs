use clickplanet_globe_protocol::{InitParams, PointerInput, PointerKind, PointerType, UiToGlobe};
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use crate::app::globe_client::GlobeClient;

fn device_pixel_ratio() -> f32 {
    web_sys::window().map(|w| w.device_pixel_ratio() as f32).unwrap_or(1.0)
}

fn pointer_input(kind: PointerKind, event: &PointerEvent) -> UiToGlobe {
    let position = event.element_coordinates();
    let pointer_type = match event.pointer_type().as_str() {
        "touch" => PointerType::Touch,
        "pen" => PointerType::Pen,
        _ => PointerType::Mouse,
    };
    UiToGlobe::Pointer(PointerInput {
        kind,
        pointer_id: event.pointer_id(),
        pointer_type,
        x: position.x as f32,
        y: position.y as f32,
    })
}

/// The globe canvas. Rendering happens in the globe bundle (see `GlobeClient`); this component
/// owns the canvas element and forwards input and size changes to it.
#[component]
pub fn Globe(client: GlobeClient, init: InitParams, attempt: u32, on_started: EventHandler<()>) -> Element {
    let mut canvas_element: Signal<Option<HtmlCanvasElement>> = use_signal(|| None);

    let on_mounted = {
        let client = client.clone();
        move |event: MountedEvent| {
            let Ok(canvas) = event.as_web_event().dyn_into::<HtmlCanvasElement>() else {
                log::error!("globe: the mounted element is not a canvas");
                return;
            };
            canvas_element.set(Some(canvas.clone()));
            if client.is_started() {
                return;
            }
            let rect = canvas.get_bounding_client_rect();
            let mut params = init.clone();
            params.width = rect.width() as f32;
            params.height = rect.height() as f32;
            params.device_pixel_ratio = device_pixel_ratio();
            client.start(canvas, params);
            on_started.call(());
        }
    };

    let send = {
        let client = client.clone();
        move |message: UiToGlobe| client.send(&message)
    };

    rsx! {
        div { class: "globe-container",
            canvas {
                key: "{attempt}",
                class: "globe-canvas",
                onmounted: on_mounted,
                onresize: {
                    let send = send.clone();
                    move |event: ResizeEvent| {
                        if let Ok(size) = event.get_border_box_size() {
                            send(UiToGlobe::Resize {
                                width: size.width as f32,
                                height: size.height as f32,
                                device_pixel_ratio: device_pixel_ratio(),
                            });
                        }
                    }
                },
                onpointerdown: {
                    let send = send.clone();
                    move |event: PointerEvent| {
                        if let Some(canvas) = canvas_element.read().as_ref() {
                            let _ = canvas.set_pointer_capture(event.pointer_id());
                        }
                        send(pointer_input(PointerKind::Down, &event));
                    }
                },
                onpointermove: {
                    let send = send.clone();
                    move |event: PointerEvent| send(pointer_input(PointerKind::Move, &event))
                },
                onpointerup: {
                    let send = send.clone();
                    move |event: PointerEvent| send(pointer_input(PointerKind::Up, &event))
                },
                onpointercancel: {
                    let send = send.clone();
                    move |event: PointerEvent| send(pointer_input(PointerKind::Cancel, &event))
                },
                onpointerleave: {
                    let send = send.clone();
                    move |event: PointerEvent| send(pointer_input(PointerKind::Leave, &event))
                },
                onwheel: {
                    let send = send.clone();
                    move |event: WheelEvent| {
                        event.prevent_default();
                        let delta_y = match event.delta() {
                            WheelDelta::Pixels(v) => v.y,
                            WheelDelta::Lines(v) => v.y * 16.0,
                            WheelDelta::Pages(v) => v.y * 800.0,
                        };
                        send(UiToGlobe::Wheel { delta_y: delta_y as f32 });
                    }
                },
                oncontextmenu: move |event: MouseEvent| event.prevent_default(),
            }
        }
    }
}
