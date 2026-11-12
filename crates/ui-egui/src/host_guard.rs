// //// Neoffice — added file (no upstream equivalent): what a page that frames the editor changes about it, and the guard that keeps its document the only one.

//! A page that frames the browser build (`?host=parent`: the Drive of the Suite, Neodesk's window) opens the document
//! itself and stores what the editor saves. The editor then has one document and one window, the page's, whatever
//! upstream lets a person open beside it ([`crate::Services::host_owns_documents`]).
//!
//! Upstream's editor keeps several documents open (File › New and Open open beside the others, View › New Window,
//! Switch Windows, a mail merge into a new document…). The rule here is the other way round from a list of what the page
//! forbids: [`switches_document`] names every command that opens, creates, closes or switches to another document, or
//! opens another window; `WordApp::run` refuses them (the page's own file arrives through the host inbox, which lifts the
//! guard for it), and [`hidden_from_a_host`] keeps them out of the command search and the Quick Access Toolbar menu.
//! A test scans the engine for the places that add, replace, switch or close a document, so a command upstream adds
//! later that does so fails it until it is named here.

use serde_json::Value;

use crate::{WordApp, i18n};

/// The commands a hosted editor refuses whatever their parameters are: they open, create, close or switch to another
/// document, or open another window. The engine opens documents in `cmd/file.rs`, `cmd/mailings/{labels,merge}.rs`,
/// `recovery.rs`, switches or closes them in `cmd/view.rs` and `cmd/file.rs`; the window is
/// `view.newWindow`.
// //// Neoffice — v0.18.1: the Language packs (`proof.packs`, `proof.installPack`, `proof.removePack`: they ask the pinned list or download a
// pack, and a hosted editor downloads nothing) and the commands that arrange or pair the windows (`view.arrangeAll`, `view.sideBySide`,
// `view.syncScroll`: the page's document is the only one, in the only window).
const REFUSED: [&str; 17] = [
    // Open beside the others, or in place of an untouched blank one.
    "file.new",
    "file.open",
    "file.newFromTemplate",
    "file.recover",
    // Close the page's document (File › Close, and the agents' `file.closeDocument` with an id).
    "file.close",
    "file.closeDocument",
    // The next save would reach the page under another name: a copy.
    "file.rename",
    // A new document of labels, and one of merged letters printed.
    "mailings.labels",
    "mailings.printDocuments",
    // Another window on the document, and the list of open documents (any of them, or none, the page's alone).
    "view.newWindow",
    "view.switchWindows",
    // //// Neoffice — the windows are arranged and paired with each other (they are disabled in a browser, and refused here).
    "view.arrangeAll",
    "view.sideBySide",
    "view.syncScroll",
    // //// Neoffice — no Language pack is listed, downloaded or removed by an editor a page hosts: nothing leaves the page.
    "proof.packs",
    "proof.installPack",
    "proof.removePack",
];

/// Whether `id` with `params`, run by an editor whose hosting page opens the documents, would open, create, close or
/// switch to another document, or open another window: once the page owns the documents, none of them is the editor's
/// to run.
pub(crate) fn switches_document(id: &str, params: &Value) -> bool {
    if REFUSED.contains(&id) {
        return true;
    }
    // An `original` file given replaces the document with it as well.
    let shown_elsewhere =
        || params.get("showIn").and_then(Value::as_str).unwrap_or("original") != "original" || params.get("original").is_some_and(|v| !v.is_null());
    match id {
        // An envelope is a new document unless it is added to this one.
        "mailings.envelopes" => params.get("add").and_then(Value::as_bool) != Some(true),
        // A merge is a new document unless it is saved to a file.
        "mailings.finish" => params.get("path").and_then(Value::as_str).is_none(),
        // A comparison shown in the revised or a new document makes the page's document an untitled one.
        "review.compare" | "review.combine" => shown_elsewhere(),
        _ => false,
    }
}

/// The commands a hosted editor does not offer (in its command search, its Quick Access Toolbar menu, its ribbon): they
/// are refused whatever the parameters are ([`switches_document`] refuses them anyway), so a button for one would only
/// answer that the page opens the documents. The mail merge ones that only add to the document (an envelope added to it,
/// a merged file saved) stay.
pub(crate) fn hidden_from_a_host(id: &str) -> bool {
    REFUSED.contains(&id)
}

/// Whether a file that reaches the editor from the user (a drop, a pick) would open as the document: not a picture, a
/// spreadsheet taken as a recipient list, a recipient list or a font. A hosting page that owns the documents drops these
/// (the web shell asks), and keeps the rest.
pub fn opens_as_document(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let picture = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".wmf", ".emf"].iter().any(|e| lower.ends_with(e));
    let sheet = [".xlsx", ".xlsm", ".xls", ".ods"].iter().any(|e| lower.ends_with(e));
    let list = lower.ends_with(".csv") || lower.ends_with(".tsv");
    !(picture || sheet || list || wordcraft_fonts::is_font_file_name(&lower))
}

// A file the person picks in the browser for a picture, a font or a recipient list arrives in the same inbox as a document
// dropped or handed over by a hosting page, and the inbox opens whatever is no picture, font or list as the document (with
// the hosting page's guard lifted for it). The browser's picker only suggests its filter (a system may let the person
// choose any file), so the web shell takes a picked file only when it is of the kind asked for.
/// Whether a file picked in the browser for `purpose` (`picture`, `font`, `recipients`; anything else is a document) is
/// of that kind, by its name as the inbox reads it: a picture is inserted, a font added, a list loaded, never a document
/// opened in their place.
pub fn picked_file_fits(purpose: &str, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    // As the inbox reads it (`is_spreadsheet_name`): a name that is only an extension (`.xlsx`) has none.
    let extension = std::path::Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or_default();
    match purpose {
        "picture" => ["png", "jpg", "jpeg", "gif", "webp", "bmp", "wmf", "emf"].contains(&extension),
        "font" => wordcraft_fonts::is_font_file_name(&lower),
        "recipients" => wordcraft_engine::cmd::mailings::RECIPIENT_EXTENSIONS.contains(&extension),
        _ => true,
    }
}

/// What the status bar says of a picked file [`picked_file_fits`] turned down.
pub fn picked_file_refused(purpose: &str, name: &str) -> String {
    let message = match purpose {
        "font" => tl!("{name} can't be added as a font: nothing was added."),
        "recipients" => tl!("{name} can't be loaded as a recipient list: nothing was loaded."),
        _ => tl!("{name} can't be inserted as a picture: nothing was inserted."),
    };
    i18n::fmt(message, &[("name", name)])
}

impl WordApp {
    /// Plain-text formats a hosting page saves in place.
    pub(crate) fn keeps_text_format(&self, ext: &str) -> bool {
        self.services.keep_text_format && matches!(ext.to_ascii_lowercase().as_str(), "txt" | "text" | "md" | "markdown")
    }

    /// A Save of a document that was opened as a text file, under that file's own name.
    pub(crate) fn is_text_saved_in_place(&self, name: &str) -> bool {
        let ext_of = |p: &std::path::Path| p.extension().map(|e| e.to_string_lossy().to_string());
        match (ext_of(std::path::Path::new(name)), self.session.path.as_deref().and_then(ext_of)) {
            (Some(saved), Some(opened)) => saved.eq_ignore_ascii_case(&opened) && self.keeps_text_format(&saved),
            _ => false,
        }
    }

    /// The community links (Discord) are shown unless the hosting page hides them.
    pub(crate) fn community_links(&self) -> bool {
        !self.services.hide_community_links
    }

    /// The hosting page opens the documents: no Home, New or Open, no other document or window
    /// ([`crate::Services::host_owns_documents`]).
    pub(crate) fn host_owns_documents(&self) -> bool {
        self.services.host_owns_documents
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_engine::Session;

    use super::*;
    use crate::{Inbox, Services};

    fn app() -> WordApp {
        WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default())
    }

    /// A hosted editor: the page opens the documents, and hands its file over through the host inbox.
    fn hosted(text: &str) -> (WordApp, Inbox, Inbox) {
        let inbox = Inbox::default();
        let host_inbox = Inbox::default();
        let services = Services { host_owns_documents: true, inbox: Some(inbox.clone()), host_inbox: Some(host_inbox.clone()), ..Default::default() };
        (WordApp::new(Session::new(wordcraft_doc::Document::from_text(text)), services), inbox, host_inbox)
    }

    // A text file opened under a hosting page is saved as text, and the document counts as saved; every other format
    // still becomes a Word document, and a page that does not host the editor changes nothing.
    #[test]
    fn hosted_text_file_is_saved_as_text() {
        let got: std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<u8>)>>> = Default::default();
        let sink = got.clone();
        let services = Services {
            keep_text_format: true,
            download: Some(Box::new(move |n: &str, b: &[u8]| {
                sink.borrow_mut().push((n.to_string(), b.to_vec()));
                Ok(())
            })),
            ..Default::default()
        };
        let mut a = WordApp::new(Session::new(wordcraft_doc::Document::from_text("Une ligne de texte")), services);
        for (opened, expect) in [("Notes.txt", "Notes.txt"), ("NOTES.TXT", "NOTES.TXT"), ("a.md", "a.md"), ("b.markdown", "b.markdown")] {
            a.session.path = Some(opened.into());
            a.session.dirty = true;
            a.run("file.save", json!({})).unwrap();
            let (name, bytes) = got.borrow_mut().pop().unwrap();
            assert_eq!(name, expect);
            assert!(String::from_utf8_lossy(&bytes).contains("Une ligne de texte"), "{opened}: the text itself");
            assert!(!a.session.dirty, "{opened}: saved under its own name is saved");
        }
        // The rest is as upstream has it: a Word document, or another format that turns into one.
        for (opened, expect) in [("c.html", "c.docx"), ("d.odt", "d.docx"), ("e.docx", "e.docx"), ("f.rtf", "f.docx"), ("g.docm", "g.docm")] {
            a.session.path = Some(opened.into());
            a.run("file.save", json!({})).unwrap();
            assert_eq!(got.borrow_mut().pop().unwrap().0, expect, "{opened}");
        }
        // Save As to another name is a copy, not the document: it stays unsaved.
        a.session.path = Some("Notes.txt".into());
        a.session.dirty = true;
        a.run("file.saveAs", json!({"path": "Autre.txt"})).unwrap();
        assert_eq!(got.borrow_mut().pop().unwrap().0, "Autre.txt");
        assert!(a.session.dirty);

        // Without the host's setting a text file turns into a Word document, as it always did.
        let sink = got.clone();
        let upstream = Services {
            download: Some(Box::new(move |n: &str, b: &[u8]| {
                sink.borrow_mut().push((n.to_string(), b.to_vec()));
                Ok(())
            })),
            ..Default::default()
        };
        let mut b = WordApp::new(Session::new(wordcraft_doc::Document::from_text("x")), upstream);
        b.session.path = Some("Notes.txt".into());
        b.run("file.save", json!({})).unwrap();
        assert_eq!(got.borrow_mut().pop().unwrap().0, "Notes.docx");
    }

    // The community links (Discord) are there unless the hosting page hides them.
    #[test]
    fn community_links_are_hidden_only_when_the_host_says_so() {
        assert!(app().community_links());
        let hidden = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services { hide_community_links: true, ..Default::default() });
        assert!(!hidden.community_links());
        // The command does nothing, rather than opening the address.
        let mut hidden = hidden;
        hidden.run("ui.discord", json!({})).unwrap();
        assert!(hidden.canvas.open_url.is_none());
        let mut shown = app();
        shown.run("ui.discord", json!({})).unwrap();
        assert_eq!(shown.canvas.open_url.as_deref(), Some("https://discord.gg/artcraft"));
    }

    // A hosting page opens the documents: the editor does not switch to another one by itself, and the file the page
    // hands over still opens.
    #[test]
    fn a_host_that_opens_the_documents_keeps_the_editor_on_its_document() {
        let (mut a, inbox, host_inbox) = hosted("Le devis du client");
        let before = a.session.document_id();
        for (id, params) in [
            ("file.new", json!({})),
            ("file.new", json!({"template": "sample"})),
            ("file.open", json!({"path": "Autre.docx"})),
            ("file.open", json!({})),
            ("file.close", json!({})),
            ("file.recover", json!({})),
            ("file.newFromTemplate", json!({"template": "letter"})),
            ("mailings.labels", json!({})),
            // v0.10.0's Rename, and a comparison shown in a new document: the page's document would be renamed, or untitled.
            ("file.rename", json!({"name": "Autre"})),
            ("review.compare", json!({"text": "Un autre texte", "showIn": "new"})),
        ] {
            assert!(a.run(id, params).is_err(), "{id} is the page's, not the editor's");
            assert_eq!(a.session.document_id(), before, "{id}: still the page's document");
        }
        let said = a.status_msg.as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
        assert!(said.contains("hosts the editor"), "{said}");
        // Even a document with unsaved changes is not asked about: nothing is going to replace it.
        a.session.dirty = true;
        assert!(a.run("file.new", json!({})).is_err());
        assert!(a.dialog.is_none());
        // A document the person picked or dropped (the shared inbox) does not lift the guard: it is refused.
        a.session.dirty = false;
        inbox.lock().unwrap().push(("Picked.txt".into(), b"Un document choisi par la personne.\n".to_vec()));
        a.drain_inbox();
        assert_eq!(a.session.document_id(), before, "a picked document is not the page's");
        // The page's own file opens, and the guard is up again after it, even with a recipient list pick left pending
        // (a `.txt` would load as a list).
        a.recipient_list_pending = true;
        host_inbox.lock().unwrap().push(("Page.txt".into(), b"Le texte que la page donne.\n".to_vec()));
        a.drain_inbox();
        assert_ne!(a.session.document_id(), before, "the page's file replaced the document");
        assert!(a.run("file.new", json!({})).is_err());
        // Home, New and Open are not pages of the File menu: asking for one shows Info, and no Start screen shows.
        a.run("ui.backstage", json!({"value": true, "page": "open"})).unwrap();
        assert_eq!(a.ui.backstage_page, "info");
        assert!(!a.start_screen());
    }

    // Every command that opens, creates, closes or switches to a document, or opens a window, is refused by a hosted
    // editor and leaves it on the page's document, whatever its parameters; the same commands do their work in an
    // editor no page hosts. This is the list of the v0.14.0 merge: upstream keeps several documents open (File › Close
    // closes only one of them, `file.closeDocument` and `view.switchWindows` name them by id, View › New Window opens a
    // window on one, a mail merge opens its letters beside it), and its own `discards_document` now names `file.close`
    // alone.
    #[test]
    fn a_hosted_editor_refuses_every_command_that_opens_creates_closes_or_switches_documents() {
        // //// Neoffice — v0.18.1 adds the window arrangements and the Language packs (a download and a list asked of the network).
        let refused: [(&str, Value); 30] = [
            ("file.new", json!({})),
            ("file.new", json!({"template": "letter"})),
            ("file.open", json!({})),
            ("file.open", json!({"path": "Autre.docx"})),
            ("file.open", json!({"path": "Autre.docx", "data": "TcSbc3Rv"})),
            ("file.newFromTemplate", json!({"path": "Modele.dotx"})),
            ("file.recover", json!({})),
            ("file.recover", json!({"restore": "abc"})),
            ("file.close", json!({})),
            ("file.close", json!({"quit": true})),
            ("file.closeDocument", json!({})),
            ("file.closeDocument", json!({"id": 1, "force": true})),
            ("file.rename", json!({"name": "Autre"})),
            ("view.switchWindows", json!({})),
            ("view.switchWindows", json!({"id": 1})),
            ("view.newWindow", json!({})),
            ("mailings.labels", json!({})),
            ("mailings.envelopes", json!({"delivery": "Quelqu'un"})),
            ("mailings.finish", json!({})),
            ("mailings.finish", json!({"from": 2, "to": 3})),
            ("mailings.printDocuments", json!({"from": 2})),
            ("review.combine", json!({"text": "Theirs", "showIn": "revised"})),
            ("view.arrangeAll", json!({})),
            ("view.sideBySide", json!({})),
            ("view.sideBySide", json!({"value": true, "window": 2})),
            ("view.syncScroll", json!({"value": true})),
            ("proof.packs", json!({})),
            ("proof.packs", json!({"refresh": true})),
            ("proof.installPack", json!({"locale": "nl"})),
            ("proof.removePack", json!({"locale": "nl"})),
        ];
        let (mut hosted_app, ..) = hosted("Le devis du client");
        let page = hosted_app.session.document_id();
        for (id, params) in &refused {
            assert!(switches_document(id, params), "{id} {params}");
            let before = hosted_app.session.documents().len();
            assert!(hosted_app.run(id, params.clone()).is_err(), "{id} {params} must be refused in a hosted editor");
            assert_eq!(hosted_app.session.document_id(), page, "{id} {params}: still the page's document");
            assert_eq!(hosted_app.session.documents().len(), before, "{id} {params}: no document opened or closed");
            assert!(hosted_app.dialog.is_none(), "{id} {params}: no dialog (a Save Changes prompt would close it)");
            assert!(hosted_app.has_document());
        }
        // The same commands are not all refused outside a hosted editor (it stays as upstream has it).
        let mut free = app();
        let first = free.session.document_id();
        free.run("file.new", json!({})).unwrap();
        assert_ne!(free.session.document_id(), first);
        assert_eq!(free.session.documents().len(), 2);
        // What only adds to the page's document is still the editor's.
        assert!(!switches_document("mailings.envelopes", &json!({"add": true, "delivery": "Quelqu'un"})));
        assert!(!switches_document("mailings.finish", &json!({"path": "out.docx"})));
        assert!(!switches_document("review.compare", &json!({"text": "Theirs"})));
        assert!(!switches_document("review.compare", &json!({"text": "Theirs", "showIn": "original"})));
        assert!(!switches_document("text.insert", &json!({"text": "x"})));
        assert!(!switches_document("view.split", &json!({})));
        let r = hosted_app.run("mailings.envelopes", json!({"add": true, "delivery": "Quelqu'un"}));
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(hosted_app.session.documents().len(), 1);
    }

    // The refused commands are not offered either: not in the command search, not in the Quick Access Toolbar menu.
    #[test]
    fn the_refused_commands_are_not_offered_by_a_hosted_editor() {
        for id in REFUSED {
            assert!(hidden_from_a_host(id), "{id}");
            assert!(app().session.registry.get(id).is_some(), "{id} is a command of the registry: renamed upstream?");
        }
        assert!(!hidden_from_a_host("file.save"));
        assert!(!hidden_from_a_host("mailings.finish"));
    }

    // The engine opens, replaces, switches and closes documents in a few places only. A command added upstream that does
    // so is a new way to leave the page's document: this fails until `REFUSED` or `switches_document` names it (and the
    // test above runs it). The list below is what v0.14.0 has, with the command that reaches each place.
    #[test]
    fn no_command_opens_or_switches_documents_that_the_guard_does_not_name() {
        let engine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/src");
        let mut found = std::collections::BTreeMap::<String, usize>::new();
        // The commands (`cmd/`) and the recovery of unsaved documents (`recovery*`, reached by `file.recover`).
        for path in crate::rust_sources(&engine).into_iter().filter(|p| {
            let name = p.strip_prefix(&engine).map(|n| n.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            (name.starts_with("cmd/") || name.starts_with("recovery")) && !name.contains("test")
        }) {
            let name = path.strip_prefix(&engine).unwrap().to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&path).unwrap();
            let n = ["add_document(", "add_opened_document(", ".set_document(", ".activate(", ".close_document(", "switch_document("]
                .iter()
                .map(|call| text.matches(call).count())
                .sum::<usize>();
            if n > 0 {
                found.insert(name, n);
            }
        }
        let expected: std::collections::BTreeMap<String, usize> = [
            // file.new, file.open (set_document, add_opened_document, activate), file.closeDocument (close_document), and, since
            // v0.18 moved it here from `cmd/tools/specs.rs`, file.newFromTemplate (set_document, add_document)
            // //// Neoffice — 7 and no `specs.rs` row: the same commands, one place fewer.
            ("cmd/file.rs", 7),
            // mailings.envelopes, mailings.labels
            ("cmd/mailings/labels.rs", 2),
            // mailings.finish, mailings.printDocuments
            ("cmd/mailings/merge.rs", 1),
            // view.switchWindows
            ("cmd/view.rs", 1),
            // file.recover
            ("recovery.rs", 2),
        ]
        .into_iter()
        .map(|(name, n)| (name.to_string(), n))
        .collect();
        assert_eq!(found, expected, "a command opens, replaces, switches or closes documents where the guard does not know: name it");
    }

    // What the web shell asks before it takes a file the person picked for a picture, a font or a list.
    #[test]
    fn a_file_picked_for_a_picture_a_font_or_a_list_is_never_taken_as_a_document() {
        for (purpose, name) in [
            ("picture", "photo.JPG"),
            ("picture", "logo.png"),
            ("picture", "drawing.emf"),
            ("font", "Inter-Regular.ttf"),
            ("font", "Family.OTF"),
            ("recipients", "people.csv"),
            ("recipients", "people.txt"),
            ("recipients", "people.xlsx"),
            ("document", "letter.docx"),
            ("document", "notes.txt"),
        ] {
            assert!(picked_file_fits(purpose, name), "{purpose}: {name}");
        }
        for (purpose, name) in [
            ("picture", "letter.docx"),
            ("picture", "notes.txt"),
            ("picture", "png"),
            ("picture", "letter.docx.png.rtf"),
            ("font", "letter.docx"),
            ("font", "picture.png"),
            ("recipients", "letter.docx"),
            ("recipients", "page.html"),
            ("recipients", ".xlsx"),
        ] {
            assert!(!picked_file_fits(purpose, name), "{purpose}: {name}");
            // What the inbox would have done with it: open it as the document.
            let message = picked_file_refused(purpose, name);
            assert!(message.contains(name) && message.contains("nothing"), "{message}");
        }
    }

    // What the web shell asks before it takes a file dropped on an editor whose page opens documents.
    #[test]
    fn a_dropped_picture_font_or_list_is_not_a_document() {
        for name in ["a.docx", "A.TXT", "notes.md", "b.rtf", "c.odt", "d.html", "dropped"] {
            assert!(opens_as_document(name), "{name}");
        }
        for name in
            ["p.PNG", "p.jpeg", "w.webp", "d.WMF", "d.emf", "t.xlsx", "t.xls", "t.ods", "people.csv", "people.TSV", "Uifont-Regular.ttf", "f.otf"]
        {
            assert!(!opens_as_document(name), "{name}");
        }
    }
}
