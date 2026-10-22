// //// Neoffice — added file (no upstream equivalent): the saves a hosting page stores, and its answers to them.

//! A page that frames the browser build (`?host=parent`: the Drive of the Suite, Neodesk's window) is handed every
//! file the editor would download (`wordcraft:save`) and answers how storing it went (`wordcraft:saved {name, ok,
//! error}`, Neoffice's addition to upstream's bridge contract, `apps/wordcraft-web/src/web/host.rs`).
//!
//! Each save the page is handed finishes later, as one in the browser's save picker does: it starts a ticket of
//! upstream's [`AsyncSaves`] (#1315) and the page's answer ends it. A refusal then takes upstream's own path
//! ([`WordApp::settle_async_saves`]): the document it was for is unsaved again and the status bar says why; and while
//! the page has not answered, the save is not confirmed (the leave-page guard warns, New and Open would ask first).
//!
//! The page has one answer a picker has not: after refusing a save (someone else saved the file since it was
//! opened), it may store those same bytes once the person has settled it there (replacing the other version, or
//! keeping a copy) and say `ok: true` again, with no save of that name still waiting. That marks the document saved
//! again, if it has not changed since that file was handed over ([`HostSaves::settle`]).
//!
//! A page that never answers leaves its saves unconfirmed: the leave-page guard keeps warning. The Drive of the Suite
//! and Neodesk answer every save, in the order they were made.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::WordApp;
use crate::async_saves::{AsyncSaves, SaveOutcome};

/// The saves handed to the hosting page and not yet answered, shared by the bridge's hooks and the shell (a cheap clone).
#[derive(Clone)]
pub struct HostSaves(Rc<State>);

struct State {
    /// The tickets the app reads ([`crate::Services::async_saves`]).
    saves: AsyncSaves,
    /// Saves handed to the page and not answered, oldest first: the ticket each one started, and its file name.
    waiting: RefCell<VecDeque<(u64, String)>>,
    /// Files handed over since the shell last settled: it notes the document's revision for them then.
    handed: RefCell<Vec<String>>,
    /// For each file name that, stored, saves the document (not an export): (revision, document id) of the document
    /// when it was last handed over under that name.
    handed_rev: RefCell<HashMap<String, (u64, u64)>>,
    /// Names the page said it stored when no save of them was waiting (a refused save the person then settled).
    confirmed: RefCell<Vec<String>>,
}

impl HostSaves {
    /// The saves the bridge hands over start their tickets in `saves`, the app's.
    pub fn new(saves: AsyncSaves) -> Self {
        Self(Rc::new(State {
            saves,
            waiting: RefCell::default(),
            handed: RefCell::default(),
            handed_rev: RefCell::default(),
            confirmed: RefCell::default(),
        }))
    }

    /// `name` was handed to the page (from the app's `download` hook): the save finishes when the page answers.
    pub fn handed(&self, name: &str) {
        let ticket = self.0.saves.start();
        self.0.waiting.borrow_mut().push_back((ticket, name.to_string()));
        self.0.handed.borrow_mut().push(name.to_string());
    }

    /// The page's answer for `name`: `Ok` when it stored it, the reason otherwise. It ends the oldest save of that name
    /// still waiting (an answer that names no file ends the oldest save). With none waiting, a stored file is the late
    /// answer of a save it refused before (see [`Self::settle`]), and a refusal is only logged: the save it was for
    /// has already ended.
    pub fn answered(&self, name: &str, result: Result<(), String>) {
        let waiting = {
            let mut waiting = self.0.waiting.borrow_mut();
            let i = if name.is_empty() { (!waiting.is_empty()).then_some(0) } else { waiting.iter().position(|(_, n)| n == name) };
            i.and_then(|i| waiting.remove(i))
        };
        match (waiting, result) {
            (Some((ticket, _)), Ok(())) => self.0.saves.finish(ticket, SaveOutcome::Saved),
            (Some((ticket, _)), Err(error)) => self.0.saves.finish(ticket, SaveOutcome::Failed(error)),
            (None, Ok(())) => self.0.confirmed.borrow_mut().push(name.to_string()),
            (None, Err(error)) => log::warn!("the hosting page refused {name}, which it had already answered for: {error}"),
        }
    }

    /// Saves handed to the page that it has not answered yet.
    pub fn waiting(&self) -> usize {
        self.0.waiting.borrow().len()
    }

    /// Called by the shell every frame, before the app's own pass: notes the revision of the document for a file just
    /// handed over that saves it, and marks the document saved again when the page stored a save of it that it had
    /// refused, if the document is still the one handed over under that name and has not changed since (a later edit
    /// is not in those bytes; an export handed over meanwhile changes nothing). The refusal's message goes with it.
    pub fn settle(&self, app: &mut WordApp) {
        let now = (app.session.rev(), app.session.document_id());
        for name in std::mem::take(&mut *self.0.handed.borrow_mut()) {
            if saves_the_document(app, &name) {
                self.0.handed_rev.borrow_mut().insert(name, now);
            }
        }
        for name in std::mem::take(&mut *self.0.confirmed.borrow_mut()) {
            if self.0.handed_rev.borrow().get(&name) == Some(&now) && saves_the_document(app, &name) {
                app.session.dirty = false;
                if app.status_msg.as_ref().is_some_and(|(message, _)| message.contains(name.as_str())) {
                    app.status_msg = None;
                }
            }
        }
    }
}

/// Whether a file of this name, stored, saves the document (as `WordApp::execute` marks it saved): a format that keeps
/// everything, or the text file a hosting page opened, saved under its own name.
fn saves_the_document(app: &WordApp, name: &str) -> bool {
    crate::keeps_everything(name) || app.is_text_saved_in_place(name)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_engine::Session;

    use super::*;
    use crate::Services;

    /// An editor hosted by a page that takes its saves, as `web/host.rs` connects it, with "Unsaved words" typed.
    fn hosted() -> (WordApp, HostSaves, AsyncSaves) {
        let saves = AsyncSaves::default();
        let host = HostSaves::new(saves.clone());
        let hook = host.clone();
        let services = Services {
            download: Some(Box::new(move |path: &str, _: &[u8]| {
                hook.handed(path);
                Ok(())
            })),
            async_saves: Some(saves.clone()),
            host_owns_documents: true,
            ..Default::default()
        };
        let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), services);
        app.run("text.insert", json!({"text": "Unsaved words"})).unwrap();
        assert!(app.session.dirty);
        (app, host, saves)
    }

    fn status(app: &WordApp) -> String {
        app.status_msg.as_ref().map(|(m, _)| m.clone()).unwrap_or_default()
    }

    #[test]
    fn a_save_the_page_stores_ends_and_leaves_the_document_saved() {
        let (mut app, host, saves) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        assert!(!app.session.dirty, "handed over, the document looks saved");
        assert_eq!((saves.pending(), host.waiting()), (1, 1), "until the page answers, the save is not confirmed");
        host.answered("letter.docx", Ok(()));
        app.settle_async_saves();
        assert_eq!((saves.pending(), host.waiting()), (0, 0));
        assert!(!app.session.dirty);
    }

    #[test]
    fn a_save_the_page_refuses_leaves_the_document_unsaved_and_says_why() {
        let (mut app, host, saves) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        app.status_msg = None;
        host.answered("letter.docx", Err("Someone else changed this document since you opened it.".into()));
        app.settle_async_saves();
        assert_eq!(saves.pending(), 0);
        assert!(app.session.dirty, "the document is unsaved again");
        let said = status(&app);
        assert!(said.contains("Couldn't save letter.docx") && said.contains("Someone else changed"), "{said:?}");
        assert!(!said.contains("download"), "nothing was downloaded: {said:?}");
    }

    #[test]
    fn a_refused_save_the_page_stores_later_is_saved_again_unless_the_document_changed() {
        for edit_after in [false, true] {
            let (mut app, host, _) = hosted();
            app.run("file.save", json!({"path": "letter.docx"})).unwrap();
            host.settle(&mut app);
            host.answered("letter.docx", Err("conflict".into()));
            app.settle_async_saves();
            assert!(app.session.dirty);
            if edit_after {
                app.run("text.insert", json!({"text": " and more"})).unwrap();
            }
            // The person replaces the other version on the page: those bytes are stored now.
            host.answered("letter.docx", Ok(()));
            host.settle(&mut app);
            assert_eq!(app.session.dirty, edit_after, "edited after the save: {edit_after}");
        }
    }

    #[test]
    fn an_export_handed_over_after_a_refused_save_does_not_make_its_late_answer_save_a_later_edit() {
        let (mut app, host, _) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        host.settle(&mut app);
        host.answered("letter.docx", Err("conflict".into()));
        app.settle_async_saves();
        app.run("text.insert", json!({"text": " and more"})).unwrap();
        app.run("file.exportPdf", json!({"path": "letter.pdf"})).unwrap();
        host.settle(&mut app);
        host.answered("letter.pdf", Ok(()));
        app.settle_async_saves();
        // The page stores the refused bytes of letter.docx, which don't have the later edit.
        host.answered("letter.docx", Ok(()));
        host.settle(&mut app);
        assert!(app.session.dirty, "the edit made after the refused save is still unsaved");
    }

    #[test]
    fn an_answer_that_names_no_file_ends_the_oldest_save() {
        let (mut app, host, saves) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        host.answered("", Err("refused".into()));
        app.settle_async_saves();
        assert_eq!((saves.pending(), host.waiting()), (0, 0));
        assert!(app.session.dirty);
    }

    #[test]
    fn each_answer_ends_the_save_of_its_own_name() {
        let (mut app, host, saves) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        app.run("file.exportPdf", json!({"path": "letter.pdf"})).unwrap();
        assert_eq!(saves.pending(), 2);
        // The export's answer first: it doesn't end the document's save.
        host.answered("letter.pdf", Ok(()));
        app.settle_async_saves();
        assert_eq!((saves.pending(), host.waiting()), (1, 1));
        assert!(!app.session.dirty);
        host.answered("letter.docx", Err("the disk is full".into()));
        app.settle_async_saves();
        assert_eq!(saves.pending(), 0);
        assert!(app.session.dirty, "the document's own save was refused");
    }

    #[test]
    fn a_refusal_with_no_save_waiting_changes_nothing() {
        let (mut app, host, _) = hosted();
        app.run("file.save", json!({"path": "letter.docx"})).unwrap();
        host.answered("letter.docx", Ok(()));
        app.settle_async_saves();
        host.answered("letter.docx", Err("late".into()));
        host.settle(&mut app);
        app.settle_async_saves();
        assert!(!app.session.dirty);
    }
}
