// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Страница в браузере: запуск окна на холсте, пакет по ссылке
//! и скачивание пакета. Параметры адреса - `share.rs`.

use std::sync::Arc;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use wasm_bindgen::JsCast;

use crate::app::SceneApp;

/// Пакет демо модуля Isoliner3D. Страница открывается с ним.
const DEMO: &[u8] = include_bytes!("../../../fixtures/scene_demo.i3dscene");

/// Текст - в консоль браузера и поверх страницы. Так видна ошибка
/// запуска и на телефоне без консоли.
pub fn show(text: &str) {
    web_sys::console::log_1(&text.into());
    if let Some(out) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("report"))
    {
        out.set_text_content(Some(text));
    }
}

fn js_err(e: wasm_bindgen::JsValue) -> String {
    format!("{e:?}")
}

/// Скачать файл по адресу. Размер ограничен, как у части пакета:
/// чужой адрес считается враждебным.
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen_futures::JsFuture;
    let window = web_sys::window().ok_or_else(|| "нет окна".to_string())?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(js_err)?
        .dyn_into()
        .map_err(|_| "нет ответа".to_string())?;
    if !resp.ok() {
        return Err(format!("ответ {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(js_err)?)
        .await
        .map_err(js_err)?;
    let arr = js_sys::Uint8Array::new(&buf);
    if u64::from(arr.length()) > scene_format::MAX_ENTRY {
        return Err("файл больше допустимого".to_string());
    }
    Ok(arr.to_vec())
}

/// Отдать файл браузеру как скачиваемый: пакет или снимок PNG.
pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| "нет документа".to_string())?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js_err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_err)?;
    let a = doc
        .create_element("a")
        .map_err(js_err)?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "нет ссылки".to_string())?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    // адрес не отзывается сразу: браузер мог ещё не начать скачивание
    Ok(())
}

/// Запуск окна на холсте `#scene` страницы `index.html`.
pub fn start() {
    std::panic::set_hook(Box::new(|info| show(&format!("Isoliner Scene: {info}"))));
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let query = crate::share::parse_query(&search);
    let Some(canvas) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("scene"))
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
    else {
        return;
    };
    // На WebGL2 egui-wgpu просит сторону текстуры 8192, видеокарта
    // телефона может дать меньше. Просим то, что видеокарта даёт.
    // Образец - страница PlanetX Lite.
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.device_descriptor = Arc::new(|adapter| {
        let limits = if adapter.get_info().backend == wgpu::Backend::Gl {
            adapter.limits()
        } else {
            wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..wgpu::Limits::default()
            }
        };
        wgpu::DeviceDescriptor {
            label: Some("isoliner-scene"),
            required_limits: limits,
            ..Default::default()
        }
    });
    let options = eframe::WebOptions {
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(setup),
            ..Default::default()
        },
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async move {
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    let mut app = SceneApp::new(cc);
                    if let Some(pose) = query.pose {
                        app.set_pose(pose);
                    }
                    if query.check {
                        app.start_check();
                    }
                    // пакет по ссылке или демо модуля
                    match query.scene {
                        Some(url) => app.fetch(url, &cc.egui_ctx),
                        None => app.open_bytes("scene_demo.i3dscene".to_string(), DEMO),
                    }
                    Ok(Box::new(app))
                }),
            )
            .await;
        if let Some(loading) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading"))
        {
            loading.remove();
        }
        if let Err(e) = started {
            show(&format!("Isoliner Scene, ошибка запуска: {e:?}"));
        }
    });
}
