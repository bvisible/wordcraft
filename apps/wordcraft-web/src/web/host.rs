// //// Neoffice — added file (no upstream equivalent): the bridge between the editor and the page that frames it.

//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! WordCraft in an `<iframe>` (the Drive of the Suite, NeoDesk's window). It speaks the contract
//! upstream's GridCraft and PdfCraft already use, so one host page can drive every Craft editor.
//! Messages are plain objects with a `type`:
//!
//! - `wordcraft:ready` (to the parent): `{ version }`, the app is listening; send the file now.
//! - `wordcraft:open` (from the parent): `{ name, bytes }`, `bytes` an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a document picked with File › Open or dropped on the window.
//! - `wordcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every file
//!   WordCraft would otherwise download: Save and Save As, and the exports (PDF, plain text…).
//!   The parent stores it, or offers it, by the file's extension.
//! - `wordcraft:saved` (from the parent): `{ name, ok, error }`. Neoffice's addition: how storing
//!   `name` went. Each save handed over is a save that finishes later, on upstream's tickets
//!   (`wordcraft_ui_egui::host_saves`, `async_saves`): until the parent answers it is not confirmed
//!   (the leave-page guard warns); `ok: false` marks the document unsaved again and says why in the
//!   status bar; a later `ok: true` for a save already refused marks it saved again when it has not
//!   changed since it was handed over (a refused save the user then settled, by replacing the other
//!   version, ends there). The Drive of the Suite and Neodesk answer every save.
//! - `wordcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `wordcraft:failed` (to the parent): `{ error }` when the app couldn't start.
//!
//! Only the parent window on the page's own origin is heard or answered. In host mode the page
//! starts on a blank document instead of File › Home (the parent sends the file to edit), and
//! `?author=<name>` is the name that signs comments and tracked changes.

use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wordcraft_ui_egui::async_saves::AsyncSaves;
use wordcraft_ui_egui::host_saves::HostSaves;
use wordcraft_ui_egui::{Inbox, Services};

/// The longest user name taken from `?author=`.
const MAX_AUTHOR_CHARS: usize = 200;

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
}

impl Bridge {
    /// The bridge, when the page asked for it (`?host=parent`) and a page of its own origin frames
    /// it; `None` otherwise, and the editor then behaves as it always did.
    pub fn requested() -> Option<Self> {
        if query_param("host").as_deref() != Some("parent") {
            return None;
        }
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = window.location().origin().ok()?;
        Some(Self { window, parent, origin, reported: Rc::default() })
    }

    /// Sends every file the editor would download to the parent, tells it when unsaved changes
    /// appear or go away, and hears its files and its answers to the saves. Each save handed over
    /// starts a ticket in `saves` (the app's) that the parent's answer ends; the shell settles the
    /// parent's late answers with what this returns.
    pub fn connect(&self, services: &mut Services, inbox: Inbox, saves: AsyncSaves, ctx: egui::Context) -> HostSaves {
        // A page that frames the editor has its own Drive and its own community: a text file is saved as text,
        // and the upstream project's links are not shown (`Services::keep_text_format`, `hide_community_links`).
        services.keep_text_format = true;
        services.hide_community_links = true;
        // //// Neoffice — and it opens the documents: no Home, New or Open in the editor, no document dropped on it
        // (`Services::host_owns_documents`).
        services.host_owns_documents = true;
        let host_saves = HostSaves::new(saves);
        self.listen(inbox, host_saves.clone(), ctx);
        let bridge = self.clone();
        let handed = host_saves.clone();
        services.download = Some(Box::new(move |path: &str, bytes: &[u8]| {
            let name = bridge.post_save(path, bytes)?;
            // Not confirmed until the parent answers; the shell notes which revision of the document this was.
            handed.handed(&name);
            Ok(())
        }));
        let bridge = self.clone();
        let own = services.on_dirty.take();
        services.on_dirty = Some(Box::new(move |dirty| {
            if let Some(own) = &own {
                own(dirty);
            }
            bridge.post_dirty(dirty);
        }));
        host_saves
    }

    /// `?author=`: the host's name for the user, which signs comments and tracked changes.
    pub fn author() -> Option<String> {
        let author: String = query_param("author")?.trim().chars().take(MAX_AUTHOR_CHARS).collect();
        (!author.is_empty()).then_some(author)
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "wordcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "wordcraft:failed".into()), ("error", error.into())]);
    }

    fn post_dirty(&self, dirty: bool) {
        if self.reported.get() != Some(dirty) && self.send(&[("type", "wordcraft:dirty".into()), ("dirty", dirty.into())]) {
            self.reported.set(Some(dirty));
        }
    }

    /// Hands `bytes` to the parent under the file name of `path`, which it returns (the name the parent's answer gives).
    fn post_save(&self, path: &str, bytes: &[u8]) -> Result<String, String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[("type", "wordcraft:save".into()), ("name", name.as_str().into()), ("bytes", buffer.clone().into())])
            .ok_or_else(|| "couldn't build the message".to_string())?;
        self.parent.post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer)).map_err(|e| reason(&e))?;
        Ok(name)
    }

    fn listen(&self, inbox: Inbox, saves: HostSaves, ctx: egui::Context) {
        let parent = self.parent.clone();
        let origin = self.origin.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &parent)) {
                return;
            }
            let data = event.data();
            match string(&data, "type").as_deref() {
                Some("wordcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "document.docx".to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                }
                Some("wordcraft:saved") => {
                    let name = string(&data, "name").unwrap_or_default();
                    if js_sys::Reflect::get(&data, &JsValue::from_str("ok")).ok().and_then(|ok| ok.as_bool()) != Some(false) {
                        saves.answered(&name, Ok(()));
                    } else {
                        let error = string(&data, "error").filter(|e| !e.trim().is_empty()).unwrap_or_else(|| "the page did not say why".to_string());
                        log::error!("the hosting page could not save {name}: {error}");
                        saves.answered(&name, Err(error));
                    }
                }
                _ => return,
            }
            ctx.request_repaint();
        });
        if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// Posts a message to the parent; false if it couldn't be sent.
    fn send(&self, fields: &[(&str, JsValue)]) -> bool {
        message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
    }
}

fn query_param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(key)
}

fn reason(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

fn message(fields: &[(&str, JsValue)]) -> Option<js_sys::Object> {
    let m = js_sys::Object::new();
    for (key, value) in fields {
        js_sys::Reflect::set(&m, &JsValue::from_str(key), value).ok()?;
    }
    Some(m)
}

fn string(data: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?.as_string()
}

/// `bytes` as an `ArrayBuffer` or `Uint8Array`.
fn bytes(data: &JsValue) -> Option<Vec<u8>> {
    let bytes = js_sys::Reflect::get(data, &JsValue::from_str("bytes")).ok()?;
    if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
        Some(array.to_vec())
    } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
        Some(js_sys::Uint8Array::new(&bytes).to_vec())
    } else {
        None
    }
}
