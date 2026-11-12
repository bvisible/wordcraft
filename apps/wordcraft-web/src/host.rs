// //// Neoffice — added file (no upstream equivalent): our own host bridge. Since v0.18.0 upstream's `host.rs` is a thin layer on
// `craft-web-host`, the bridge shared by the Craft apps; this file is the one the Neoffice hosts (the Suite's Drive, Neodesk's window) talk to,
// with the answers to a save, the page's file id and the page's inbox that the shared bridge has no equivalent for (NEOFFICE_FORK_MARKERS.md).
//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! WordCraft in an `<iframe>` and keep its files (a Nextcloud app, a personal file server). The
//! same messages as GridCraft's and PdfCraft's bridges; each is a plain object with a `type`:
//!
//! - `wordcraft:ready` (to the parent): `{ version }`, sent once the app is running and
//!   listening; send a file now.
//! - `wordcraft:open` (from the parent): `{ name, bytes }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a document picked with File › Open or dropped on the window.
//!   //// Neoffice — it arrives in an inbox of its own (`Services::host_inbox`): it is the only file that may replace
//!   the document, a document the person picks or drops is refused (`Services::host_owns_documents`). And one more
//!   field, `id`: the page's own name for the file (the Drive's file id), which the Nora pane tells Nora with each
//!   message about that document (`Services::nora_file`); a page that sends none changes nothing.
//! - `wordcraft:save` (to the parent): `{ name, saveAs, bytes }` (a transferred `ArrayBuffer`) for
//!   every file WordCraft would otherwise download. The parent stores it and reports failures
//!   itself. `saveAs` says whether the file becomes the document's file:
//!   - Save: `false`. It writes the opened file under its own name and format, as the desktop
//!     app does. A name the parent never sent (an untitled document, an imported .doc saved as
//!     .docx) is a new file: `true`, as for Save As.
//!   - Save As, and File › Export to another document format (which runs Save As): `true` for a
//!     format that keeps everything (.docx and the other Word packages, .odt, .rtf). The document
//!     is now that file; the next Save sends its name with `false`.
//!   - Save As or Export to a format that doesn't (PDF, Markdown, plain text), Export as PDF and
//!     Export Page as PNG: `false`. They are copies; the document's file stays the same.
//! - `wordcraft:saved` (from the parent): `{ name, ok, error }`. //// Neoffice's addition, upstream's bridge has no
//!   answer to a save: how storing `name` went. Each save handed over is a save that finishes later, on upstream's
//!   tickets (`wordcraft_ui_egui::host_saves`, `async_saves`): until the parent answers it is not confirmed (the
//!   leave-page guard of the parent warns); `ok: false` marks the document unsaved again and says why in the status
//!   bar; a later `ok: true` for a save already refused marks it saved again when it has not changed since it was
//!   handed over (a refused save the user then settled, by replacing the other version, ends there). The Drive of the
//!   Suite and Neodesk answer every save.
//! - `wordcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `wordcraft:failed` (to the parent): `{ error }` when the app couldn't start.
//!
//! Only the parent window on the page's own origin is heard or answered. In host mode the parent
//! guards leaving the page, and `?author=` names the user for comments and tracked changes.

use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
// //// Neoffice — the tickets of the saves handed to the page, and the page's file named for Nora's pane.
use wordcraft_ui_egui::async_saves::AsyncSaves;
use wordcraft_ui_egui::host_saves::HostSaves;
use wordcraft_ui_egui::{Inbox, NoraFile, Services, WordApp};

/// The longest user name taken from `?author=`.
const MAX_AUTHOR_CHARS: usize = 200;

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
}

impl Bridge {
    /// `None` when the page isn't framed (there is no parent to talk to), or when the parent is
    /// on another origin: messages only go to this origin, so the browser would drop every save
    /// without an error while the document showed as saved.
    pub fn new() -> Option<Self> {
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = window.location().origin().ok()?;
        // An opaque origin (a `file://` page in some browsers) can't be a `postMessage` target.
        if origin == "null" {
            return None;
        }
        // Reading another origin's location throws, which is `None` too.
        if parent.location().origin().ok()? != origin {
            return None;
        }
        Some(Self { window, parent, origin })
    }

    /// Hand saved and exported files to the parent instead of downloading them, report unsaved
    /// changes to it, and hear the files it sends.
    // //// Neoffice — and its answers to the saves: each save handed over starts a ticket in `saves` (the app's) that the
    // parent's answer ends; the shell settles the parent's late answers with what this returns.
    pub fn connect(&self, services: &mut Services, saves: AsyncSaves, ctx: egui::Context) -> HostSaves {
        // //// Neoffice — a page that frames the editor has its own Drive and its own community: a text file is saved as
        // text, and the upstream project's links are not shown (`Services::keep_text_format`, `hide_community_links`).
        services.keep_text_format = true;
        services.hide_community_links = true;
        // //// Neoffice — and it opens the documents: no Home, New or Open in the editor, no other document opened or
        // closed, no other window (`Services::host_owns_documents`).
        services.host_owns_documents = true;
        let host_saves = HostSaves::new(saves);
        // //// Neoffice — the page's name for each file it sends, which Nora's pane reads (`nora_pane::named_file`).
        let file = NoraFile::default();
        services.nora_file = Some(file.clone());
        // //// Neoffice — the page's files go to their own inbox: only they open with the guard of `host_owns_documents`
        // lifted, never a document the person picks or drops (`Services::host_inbox`).
        let inbox = Inbox::default();
        services.host_inbox = Some(inbox.clone());
        self.listen(inbox, file, host_saves.clone(), ctx);
        let bridge = self.clone();
        let handed = host_saves.clone();
        services.host_save = Some(Box::new(move |name: &str, bytes: &[u8], save_as: bool| {
            let name = bridge.post_save(name, bytes, save_as)?;
            // //// Neoffice — not confirmed until the parent answers; the shell notes which revision of the document this was.
            handed.handed(&name);
            Ok(())
        }));
        // Anything else that would download goes to the parent too.
        let bridge = self.clone();
        let handed = host_saves.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| {
            let name = bridge.post_save(name, bytes, false)?;
            // //// Neoffice — as above.
            handed.handed(&name);
            Ok(())
        }));
        // //// Neoffice — `services.async_saves` stays the app's: upstream's bridge clears it (no save finishes later), ours
        // finishes each save when the parent answers it.
        let bridge = self.clone();
        let reported: Rc<Cell<Option<bool>>> = Rc::default();
        services.on_dirty = Some(Box::new(move |dirty: bool| {
            if reported.get() != Some(dirty) && bridge.send(&[("type", "wordcraft:dirty".into()), ("dirty", dirty.into())]) {
                reported.set(Some(dirty));
            }
        }));
        // //// Neoffice — the shell settles the parent's late answers with it (`host_saves`).
        host_saves
    }

    /// `?author=`: the host's name for the user signs comments and tracked changes.
    pub fn set_author(app: &mut WordApp, author: &str) {
        let author: String = author.trim().chars().take(MAX_AUTHOR_CHARS).collect();
        if !author.is_empty() {
            app.session.author = author;
        }
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "wordcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    /// The app couldn't start (no WebGPU or WebGL2, for example): let the parent say so.
    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "wordcraft:failed".into()), ("error", error.into())]);
    }

    // //// Neoffice — returns the file name handed over (the name the parent's answer gives), for the ticket.
    fn post_save(&self, path: &str, bytes: &[u8], save_as: bool) -> Result<String, String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[
            ("type", "wordcraft:save".into()),
            ("name", name.as_str().into()),
            ("saveAs", save_as.into()),
            ("bytes", buffer.clone().into()),
        ])
        .ok_or_else(|| "couldn't build the message".to_string())?;
        // //// Neoffice — the name handed over is the Ok value, for the ticket (see above).
        self.parent
            .post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer))
            .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
        Ok(name)
    }

    /// Hear `wordcraft:open` from the parent and queue the file like a dropped one.
    // //// Neoffice — and `wordcraft:saved`, its answers to the saves.
    fn listen(&self, inbox: Inbox, file: NoraFile, saves: HostSaves, ctx: egui::Context) {
        let parent = self.parent.clone();
        let origin = self.origin.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &parent)) {
                return;
            }
            let data = event.data();
            // //// Neoffice — a match, for the parent's second message (`wordcraft:saved`) beside `wordcraft:open`.
            match string(&data, "type").as_deref() {
                Some("wordcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Document.docx".to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    // //// Neoffice — the page's name for the file (the Drive's file id), for Nora's pane.
                    *file.lock().unwrap_or_else(|e| e.into_inner()) = Some((name.clone(), string(&data, "id")));
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                }
                // //// Neoffice — how storing a file went: it ends the ticket of the save (`host_saves`).
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
                // //// Neoffice — any other message is not ours to hear (upstream's bridge heard `wordcraft:open` alone).
                _ => return,
            }
            ctx.request_repaint();
        });
        if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// Post a message to the parent; false if it couldn't be sent.
    fn send(&self, fields: &[(&str, JsValue)]) -> bool {
        message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
    }
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
