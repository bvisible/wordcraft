// //// Neoffice — added file (no upstream equivalent): the connection of the Nora pane in the browser.

//! The Nora pane's connection for the browser build: a `fetch` made with the page's own session.
//!
//! Nora lives on the Neoffice server, so the editor needs the session of whoever hosts it. The page
//! that hosts the editor turns the connection on in its URL: `?nora` (calls go to the page's own
//! origin, as for an editor served by the instance itself) or `?nora=<origin>` (to another origin).
//! The pane then opens by itself, as the browser build keeps no setting between visits.
//!
//! The host can also describe the connection in `window.wordcraftNora`, read at every call:
//! `{ origin?: string, headers?: { [name]: value }, credentials?: "include" | "same-origin" | "omit" }`.
//! A Frappe page's CSRF token (`window.csrf_token` or `frappe.csrf_token`) is sent as
//! `X-Frappe-CSRF-Token` without being asked. Nothing secret travels in the URL.
//!
//! A call that gets no answer from the network comes back as a failure the pane can show.

use std::sync::{Arc, Mutex};

use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wordcraft_ui_egui::{HttpRequest, HttpResponse, NoraInbox, Services};

/// The origin Nora is asked at when the page asked for her: `""` for the page's own origin. `None`
/// when the page didn't ask. `query` is the page's `location.search`.
pub fn requested(query: &str) -> Option<String> {
    for item in query.trim_start_matches('?').split('&') {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        if key == "nora" {
            let decoded = js_sys::decode_uri_component(value).ok().and_then(|text| text.as_string()).unwrap_or_default();
            return Some(if matches!(decoded.as_str(), "1" | "true") { String::new() } else { decoded });
        }
    }
    None
}

/// Gives the app the means to reach Nora: each call is a `fetch`, each answer is delivered to the pane
/// and wakes the interface.
pub fn connect(services: &mut Services, origin: String, ctx: egui::Context) {
    let inbox: NoraInbox = Arc::new(Mutex::new(Vec::new()));
    services.nora_inbox = Some(inbox.clone());
    services.nora_http = Some(Box::new(move |request: HttpRequest| {
        let (inbox, ctx, origin) = (inbox.clone(), ctx.clone(), origin.clone());
        spawn_local(async move {
            let response = call(&origin, &request).await;
            inbox.lock().unwrap_or_else(|e| e.into_inner()).push(response);
            ctx.request_repaint();
        });
    }));
}

/// One call, answered whatever happens: a network failure becomes a refusal with its reason, which the
/// session shows like any other.
async fn call(default_origin: &str, request: &HttpRequest) -> HttpResponse {
    match fetch(default_origin, request).await {
        Ok((status, body)) => HttpResponse { id: request.id, status, body },
        Err(error) => {
            let body = serde_json::json!({ "exception": format!("No answer from Nora: {error}") }).to_string();
            HttpResponse { id: request.id, status: 0, body }
        }
    }
}

/// What the host said in `window.wordcraftNora`.
struct Host {
    origin: Option<String>,
    headers: Vec<(String, String)>,
    credentials: Option<web_sys::RequestCredentials>,
}

fn get(target: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(target, &JsValue::from_str(key)).ok().filter(|value| !value.is_undefined() && !value.is_null())
}

fn host(window: &web_sys::Window) -> Host {
    let mut host = Host { origin: None, headers: Vec::new(), credentials: None };
    if let Some(settings) = get(window, "wordcraftNora").filter(JsValue::is_object) {
        host.origin = get(&settings, "origin").and_then(|value| value.as_string());
        host.credentials = get(&settings, "credentials").and_then(|value| value.as_string()).and_then(|name| match name.as_str() {
            "include" => Some(web_sys::RequestCredentials::Include),
            "same-origin" => Some(web_sys::RequestCredentials::SameOrigin),
            "omit" => Some(web_sys::RequestCredentials::Omit),
            _ => None,
        });
        if let Some(headers) = get(&settings, "headers").filter(JsValue::is_object) {
            for entry in js_sys::Object::entries(&js_sys::Object::from(headers)).iter() {
                let pair = js_sys::Array::from(&entry);
                if let (Some(name), Some(value)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                    host.headers.push((name, value));
                }
            }
        }
    }
    host
}

/// The CSRF token of a Frappe page, if the page has one.
fn csrf_token(window: &web_sys::Window) -> Option<String> {
    get(window, "csrf_token")
        .and_then(|token| token.as_string())
        .or_else(|| get(window, "frappe").and_then(|frappe| get(&frappe, "csrf_token")).and_then(|token| token.as_string()))
        .filter(|token| !token.is_empty())
}

fn reason(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::Reflect::get(&error, &JsValue::from_str("message")).ok().and_then(|message| message.as_string()))
        .unwrap_or_else(|| format!("{error:?}"))
}

async fn fetch(default_origin: &str, request: &HttpRequest) -> Result<(u16, String), String> {
    let window = web_sys::window().ok_or("no window")?;
    let host = host(&window);
    let origin = host.origin.as_deref().unwrap_or(default_origin).trim_end_matches('/');
    let url = format!("{origin}{}", request.path);
    let headers = web_sys::Headers::new().map_err(reason)?;
    headers.set("Content-Type", "application/json").map_err(reason)?;
    headers.set("Accept", "application/json").map_err(reason)?;
    if let Some(token) = csrf_token(&window) {
        headers.set("X-Frappe-CSRF-Token", &token).map_err(reason)?;
    }
    for (name, value) in &host.headers {
        headers.set(name, value).map_err(reason)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&request.body.to_string()));
    if let Some(credentials) = host.credentials {
        init.set_credentials(credentials);
    }
    let answer = JsFuture::from(window.fetch_with_str_and_init(&url, &init)).await.map_err(reason)?;
    let response: web_sys::Response = answer.dyn_into().map_err(|_| "the answer is not a response".to_string())?;
    let text = JsFuture::from(response.text().map_err(reason)?).await.map_err(reason)?;
    Ok((response.status(), text.as_string().unwrap_or_default()))
}
