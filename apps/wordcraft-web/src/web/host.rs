// //// Neoffice — added file (no upstream equivalent): the bridge between the editor and the page that hosts it.

//! `window.wordcraftHost`: how the page that hosts the editor (the Drive of the Suite, NeoDesk's
//! window, any page that embeds it) gives it a file and receives what it saves, with no file dialog.
//!
//! The editor defines the object when it starts and tells the page with a `wordcraft:ready` event on
//! `window` (and `wordcraftHost.ready === true`). The page, from the same origin as the editor, uses:
//!
//! - `wordcraftHost.open(name, bytes)`: open a document. `name` carries its extension (`devis.docx`),
//!   `bytes` is a `Uint8Array` or an `ArrayBuffer`.
//! - `wordcraftHost.onSave = async (name, bytes) => { … }`: set by the page. When it is a function, Save
//!   calls it instead of the browser's save dialog or download, with the file's name and a
//!   `Uint8Array`. The returned promise settles the save: a rejection tells the editor the document
//!   is not saved, and it says so. The same function receives the exports (PDF, picture…), under their
//!   own names, for the page to keep or offer.
//! - `wordcraftHost.onDirty = (dirty) => { … }`: set by the page; called when the document gains or
//!   loses unsaved changes.
//!
//! Without `onSave` the editor behaves as it always did.

use std::rc::Rc;

use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wordcraft_ui_egui::Inbox;

use super::PickerSaves;

fn key(name: &str) -> JsValue {
    JsValue::from_str(name)
}

fn reason(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::Reflect::get(error, &key("message")).ok().and_then(|message| message.as_string()))
        .unwrap_or_else(|| format!("{error:?}"))
}

/// The page's `wordcraftHost` object, if the editor has made it.
fn host() -> Option<JsValue> {
    let window = web_sys::window()?;
    js_sys::Reflect::get(&window, &key("wordcraftHost")).ok().filter(JsValue::is_object)
}

/// A function the page set on the host object (`onSave`, `onDirty`).
fn handler(name: &str) -> Option<(JsValue, js_sys::Function)> {
    let host = host()?;
    let function = js_sys::Reflect::get(&host, &key(name)).ok()?.dyn_into::<js_sys::Function>().ok()?;
    Some((host, function))
}

/// Makes `window.wordcraftHost` and says the editor is ready.
pub fn install(inbox: Inbox, ctx: egui::Context) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let host = js_sys::Object::new();
    let open = Closure::<dyn FnMut(JsValue, JsValue)>::new(move |name: JsValue, bytes: JsValue| {
        let Some(name) = name.as_string() else {
            log::error!("wordcraftHost.open(name, bytes): `name` must be a string");
            return;
        };
        let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
        ctx.request_repaint();
    });
    js_sys::Reflect::set(&host, &key("open"), open.as_ref()).map_err(|e| reason(&e))?;
    // The object lives as long as the page.
    open.forget();
    js_sys::Reflect::set(&host, &key("ready"), &JsValue::TRUE).map_err(|e| reason(&e))?;
    js_sys::Reflect::set(&window, &key("wordcraftHost"), &host).map_err(|e| reason(&e))?;
    let ready = web_sys::Event::new("wordcraft:ready").map_err(|e| reason(&e))?;
    window.dispatch_event(&ready).map_err(|e| reason(&e))?;
    Ok(())
}

/// Hands a save to the page, when it asked for them: `Some` with how it went, `None` when the page set
/// no `onSave`. Like a save through the browser's picker, the document counts as saved only once
/// the page's promise settles; until then the leave-page guard keeps warning.
pub fn save(saves: &Rc<PickerSaves>, ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<Result<(), String>> {
    let (host, on_save) = handler("onSave")?;
    let result = match on_save.call2(&host, &key(name), &js_sys::Uint8Array::from(bytes)) {
        Ok(result) => result,
        Err(error) => return Some(Err(reason(&error))),
    };
    let settled = js_sys::Promise::resolve(&result);
    saves.pending.set(saves.pending.get().saturating_add(1));
    let (saves, ctx, name) = (saves.clone(), ctx.clone(), name.to_string());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = wasm_bindgen_futures::JsFuture::from(settled).await {
            let error = reason(&error);
            log::error!("the hosting page could not save {name}: {error}");
            saves.refused.borrow_mut().push((name, error));
        }
        saves.pending.set(saves.pending.get().saturating_sub(1));
        ctx.request_repaint();
    });
    Some(Ok(()))
}

/// Tells the page the document gained or lost unsaved changes.
pub fn dirty(dirty: bool) {
    if let Some((host, on_dirty)) = handler("onDirty")
        && let Err(error) = on_dirty.call1(&host, &JsValue::from_bool(dirty))
    {
        log::error!("the hosting page's onDirty failed: {}", reason(&error));
    }
}
