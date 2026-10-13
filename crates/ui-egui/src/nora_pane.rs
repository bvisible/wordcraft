// //// Neoffice — added file (no upstream equivalent): the Nora assistant pane, built on craft-nora and craft-nora-ui.

//! The Nora pane: Neoffice's assistant, in a pane at the right of the page.
//!
//! Nora (the agent, her memory and her tools) lives on the Neoffice server. This pane is her chat
//! window: it keeps a [`NoraSession`] (the conversation, from `craft-nora`) and draws it with
//! [`NoraPanel`] (from `craft-nora-ui`). It is off until File › Options › Advanced › Assistant, or
//! the `ui.nora` command, turns it on.
//!
//! The pane never opens a connection itself. The host makes the HTTP calls with its own session and
//! credentials ([`crate::Services::nora_http`]) and delivers the answers
//! ([`crate::Services::nora_inbox`]). Without a connection, which is every build today, the pane says
//! that Nora isn't available and offers no text box. What Nora asks for in the document (`doc_*`
//! actions) is not run yet either: she is told it isn't available, so a host that connects the pane
//! before the actions exist gets a polite no instead of a hang.

use craft_nora::{
    AppKind, DocActionKind, DocumentInfo, HttpRequest, HttpResponse, Input, NoraSession, Output, PageContext, Selection, SessionConfig,
};
use craft_nora_ui::{NoraLabels, NoraPanel, NoraStyle, PanelAction, drive};
use egui::Ui;

use crate::WordApp;

/// The most characters of the selection told to Nora with a message; she reads the document herself
/// when she needs more.
const MAX_SELECTION_CHARS: usize = 2_000;

/// What the pane keeps between frames: the conversation and the panel that draws it.
pub struct NoraState {
    session: NoraSession,
    panel: NoraPanel,
}

impl Default for NoraState {
    fn default() -> Self {
        let context = PageContext::editor(AppKind::Word, DocumentInfo::named(""));
        NoraState { session: NoraSession::new(SessionConfig::default(), context), panel: NoraPanel::new() }
    }
}

/// Draws the pane's content (inside the pane's frame) and moves the conversation along.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let mut outputs = deliver_answers(app);
    outputs.extend(drive(ui.ctx(), &mut app.nora.session));
    app.nora.panel.labels = labels();
    app.nora.panel.style = style(ui);
    app.nora.panel.offline = app.services.nora_http.is_none().then(|| tl!("Nora isn't available in this version yet").to_string());
    for action in app.nora.panel.show(ui, &app.nora.session) {
        if matches!(action, PanelAction::Send(_)) {
            // Where the user is, as of the message: told to Nora with it, never read on every frame.
            let context = context_of(app);
            outputs.extend(app.nora.session.handle(Input::SetContext(context)));
        }
        outputs.extend(app.nora.session.handle(action.into_input()));
    }
    perform(app, outputs, ui.ctx());
}

/// The answers the host delivered since the last frame, fed to the conversation.
fn deliver_answers(app: &mut WordApp) -> Vec<Output> {
    let Some(inbox) = app.services.nora_inbox.clone() else { return Vec::new() };
    let answers = std::mem::take(&mut *inbox.lock().unwrap_or_else(|e| e.into_inner()));
    answers.into_iter().flat_map(|answer| app.nora.session.handle(Input::Http(answer))).collect()
}

/// Does what the conversation asks: a call goes to the host, a document action is refused for now,
/// a change repaints. The refusals and the missing-connection answers come back as inputs and may ask
/// for more, so this goes round a few times at most.
fn perform(app: &mut WordApp, outputs: Vec<Output>, ctx: &egui::Context) {
    let mut queue = outputs;
    for _ in 0..8 {
        if queue.is_empty() {
            return;
        }
        let mut next = Vec::new();
        for output in std::mem::take(&mut queue) {
            match output {
                Output::Http(request) => next.extend(send(app, request)),
                Output::Document(call) => {
                    let refusal = tl!("Nora can't change the document in this version yet").to_string();
                    next.extend(app.nora.session.handle(Input::DocumentResult { id: call.id, result: Err(refusal) }));
                }
                Output::Changed => ctx.request_repaint(),
            }
        }
        queue = next;
    }
}

/// Hands one call to the host. Without a connection the call fails at once, as an unavailable server.
fn send(app: &mut WordApp, request: HttpRequest) -> Vec<Output> {
    match &app.services.nora_http {
        Some(http) => {
            http(request);
            Vec::new()
        }
        None => app.nora.session.handle(Input::Http(HttpResponse { id: request.id, status: 503, body: String::new() })),
    }
}

/// Where the user is: the editor, the document's name and page count, and the selected text.
fn context_of(app: &mut WordApp) -> PageContext {
    let name = app.session.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| app.title_stem());
    let pages = u32::try_from(app.session.layout().pages.len()).ok();
    let selected = app.session.selected_text();
    let selection = (!selected.is_empty()).then(|| Selection { text: selected.chars().take(MAX_SELECTION_CHARS).collect() });
    PageContext::editor(AppKind::Word, DocumentInfo { name, id: None, pages, selection })
}

/// The pane's bubbles in WordCraft's own colors, light or dark.
fn style(ui: &Ui) -> NoraStyle {
    let t = crate::theme::Tokens::get(ui.ctx());
    NoraStyle { user_fill: Some(t.accent), user_text: Some(t.on_accent), nora_fill: Some(t.hover), nora_text: Some(t.text), ..NoraStyle::default() }
}

/// The words of the pane in the interface language. The pane's title bar is WordCraft's own, so the
/// panel's is left empty.
fn labels() -> NoraLabels {
    NoraLabels {
        title: String::new(),
        new_conversation: tl!("New conversation").to_string(),
        undo: tl!("Undo").to_string(),
        undo_hint: tl!("Undo the last change Nora made to the document").to_string(),
        placeholder: tl!("Ask Nora…").to_string(),
        send: tl!("Send").to_string(),
        working: tl!("Nora is working…").to_string(),
        empty: tl!("Ask Nora to write, rewrite, translate or format your document.").to_string(),
        confirm_heading: tl!("Nora wants to:").to_string(),
        allow: tl!("Allow").to_string(),
        refuse: tl!("Refuse").to_string(),
        describe,
    }
}

/// One sentence for an action waiting for the user's yes, in the interface language.
fn describe(kind: &DocActionKind) -> String {
    use crate::i18n::fmt;
    match kind {
        DocActionKind::Observe => tl!("Read the document").to_string(),
        DocActionKind::Execute { command, .. } => fmt(tl!("Run {command}"), &[("command", command)]),
        DocActionKind::Batch { commands } => fmt(tl!("Run {count} commands"), &[("count", &commands.len().to_string())]),
        DocActionKind::SelectText { text, .. } => fmt(tl!("Select \"{text}\""), &[("text", &shorten(text))]),
        DocActionKind::TypeText { text, .. } => fmt(tl!("Type \"{text}\""), &[("text", &shorten(text))]),
        DocActionKind::RenderPage { page, .. } => fmt(tl!("Look at page {page}"), &[("page", &page.to_string())]),
        DocActionKind::Save => tl!("Save the document").to_string(),
        DocActionKind::Undo => tl!("Undo the last change").to_string(),
    }
}

/// The start of a text, for a sentence that quotes it.
fn shorten(text: &str) -> String {
    let mut short: String = text.chars().take(40).collect();
    if text.chars().count() > 40 {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use egui_kittest::kittest::Queryable;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use wordcraft_engine::Session;

    type Outbox = Arc<Mutex<Vec<HttpRequest>>>;

    fn app(services: Services) -> WordApp {
        WordApp::new(Session::new(wordcraft_doc::Document::new()), services)
    }

    /// A host that records the calls and lets the test answer them.
    fn connected() -> (Services, Outbox, crate::NoraInbox) {
        let outbox: Outbox = Arc::default();
        let inbox: crate::NoraInbox = Arc::default();
        let sent = outbox.clone();
        let services = Services {
            nora_http: Some(Box::new(move |request| sent.lock().unwrap_or_else(|e| e.into_inner()).push(request))),
            nora_inbox: Some(inbox.clone()),
            ..Services::default()
        };
        (services, outbox, inbox)
    }

    fn harness(app: WordApp) -> egui_kittest::Harness<'static, WordApp> {
        egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        )
    }

    fn answer(inbox: &crate::NoraInbox, request: &HttpRequest, message: Value) {
        let body = json!({ "message": message }).to_string();
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(HttpResponse { id: request.id, status: 200, body });
    }

    fn take(outbox: &Outbox) -> Vec<HttpRequest> {
        std::mem::take(&mut *outbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// The Nora pane is a UI command, off by default and kept between runs.
    #[test]
    fn nora_is_a_ui_command_kept_between_runs() {
        let mut a = app(Services::default());
        assert_eq!(a.run("ui.nora", json!({})).unwrap(), json!({"nora": false}), "off by default; no value reads it");
        assert_eq!(a.run("ui.nora", json!({"value": true})).unwrap(), json!({"nora": true}));
        for junk in [json!({"value": "yes"}), json!({"value": 1}), json!({"value": null}), json!({"value": [true]})] {
            assert!(a.run("ui.nora", junk).is_err());
        }
        assert!(a.ui.nora_pane, "a rejected value keeps the setting");
        let saved = serde_json::to_string(&a.prefs()).unwrap();
        assert!(saved.contains(r#""noraPane":true"#), "{saved}");
        let mut b = app(Services::default());
        b.apply_prefs(serde_json::from_str(&saved).unwrap());
        assert!(b.ui.nora_pane);
        // Settings saved before the pane existed leave it off.
        let old: crate::UiState = serde_json::from_str(r#"{"tab":"Insert"}"#).unwrap();
        assert!(!old.nora_pane);
    }

    /// File › Options › Advanced › Assistant is the `ui.nora` setting.
    #[test]
    fn the_options_checkbox_shows_and_hides_the_pane() {
        let mut a = app(Services::default());
        a.run("file.options", json!({"pane": "advanced"})).unwrap();
        let mut h = harness(a);
        h.run_steps(4);
        assert!(h.query_by_label("Assistant").is_some(), "the section is there");
        h.get_by_label("Show the Nora assistant pane (experimental)").click();
        h.run_steps(4);
        assert!(h.state().ui.nora_pane, "the checkbox turns it on");
        h.get_by_label("Show the Nora assistant pane (experimental)").click();
        h.run_steps(4);
        assert!(!h.state().ui.nora_pane, "and off again");
    }

    /// Nothing is drawn while the pane is off, and without a connection the pane says why it can't
    /// be used instead of offering a text box.
    #[test]
    fn without_a_connection_the_pane_says_nora_isnt_available() {
        let mut h = harness(app(Services::default()));
        h.run_steps(3);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none(), "off by default");
        h.state_mut().run("ui.nora", json!({"value": true})).unwrap();
        h.run_steps(4);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_some());
        assert!(h.query_by_role(egui::accesskit::Role::MultilineTextInput).is_none(), "no text box to lose a message in");
        // The title bar's close button is a painted icon that runs this very command.
        h.state_mut().run("ui.nora", json!({"value": false})).unwrap();
        h.run_steps(4);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none(), "closed, it draws nothing");
    }

    /// A message typed in the pane leaves as the first call of the conversation, carrying where the
    /// user is; the host's answers show up in the pane; the document is left alone.
    #[test]
    fn a_connected_pane_sends_the_message_with_its_context_and_shows_the_reply() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.run_steps(4);
        assert!(h.query_by_label("Nora isn't available in this version yet").is_none());

        h.get_by_role(egui::accesskit::Role::MultilineTextInput).focus();
        h.run_steps(2);
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("Rewrite the intro");
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(4);

        let calls = take(&outbox);
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].path.ends_with("nora.api.chat.start_or_resume_thread"), "{}", calls[0].path);
        assert_eq!(calls[0].body["message"], "Rewrite the intro");
        let context: Value = serde_json::from_str(calls[0].body["context"].as_str().unwrap_or_default()).unwrap();
        assert_eq!(context["surface"], "editor");
        assert_eq!(context["app"], "word");
        assert_eq!(context["document"]["name"], "Document1");
        assert_eq!(context["document"]["pages"], 1);
        assert!(h.query_by_label("Rewrite the intro").is_some(), "the user's own message is in the transcript");
        assert_eq!(h.state().session.word_count(), 0, "typing in the pane never edits the document");

        // The server gives the conversation an id; the pane then reads the replies and the actions.
        answer(&inbox, &calls[0], json!({"thread_id": "t1", "sent_at": 1000.0}));
        h.run_steps(6);
        let polls = take(&outbox);
        let reads = polls.iter().find(|r| r.path.ends_with("get_messages")).expect("the replies are read");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        // As the real server stamps them: an older reply is not Nora's answer to this message.
        let replies = json!([
            {"role": "assistant", "text": "An answer from an earlier conversation.", "ts": 900.0},
            {"role": "assistant", "text": "Done, the intro is friendlier.", "ts": 1001.0}
        ]);
        answer(&inbox, reads, json!({"messages": replies}));
        answer(&inbox, actions, json!({"actions": []}));
        h.run_steps(4);
        assert!(h.query_by_label("Done, the intro is friendlier.").is_some(), "Nora's reply is shown");
        assert!(h.query_by_label("An answer from an earlier conversation.").is_none(), "a reply older than the send is not");
    }

    /// What Nora asks for in the document is refused for now, and she is told.
    #[test]
    fn document_actions_are_refused_and_nora_is_told() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Bold the title");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        let first = take(&outbox);
        answer(&inbox, &first[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        let polls = take(&outbox);
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        answer(&inbox, actions, json!({"actions": [{"action": "doc_execute", "command": "format.bold", "callback_id": "c1"}]}));
        h.run_steps(4);
        let told = take(&outbox);
        let observation = told.iter().find(|r| r.path.ends_with("receive_observation")).expect("Nora gets an answer");
        assert_eq!(observation.body["callback_id"], "c1");
        assert_eq!(observation.body["observation"]["ok"], false);
    }

    /// The sentences of the pane are words of the interface language, not the core's English.
    #[test]
    fn actions_are_described_with_the_pane_words() {
        assert_eq!(describe(&DocActionKind::Save), "Save the document");
        assert_eq!(describe(&DocActionKind::Execute { command: "format.bold".into(), params: json!({}) }), "Run format.bold");
        let long = "é".repeat(60);
        let typed = describe(&DocActionKind::TypeText { text: long, paragraphs: false });
        assert!(typed.starts_with("Type \"") && typed.ends_with("…\""), "{typed}");
    }
}
