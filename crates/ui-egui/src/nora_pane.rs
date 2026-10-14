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
//! that Nora isn't available and offers no text box.
//!
//! What Nora asks for in the document (`doc_*` actions) is played here, with the vocabulary of
//! WordCraft's own MCP tools (`execute`, `batch`, `select_text`, `type_text`, `get_text`,
//! `render_page`, `save_document`) through [`WordApp::execute`], which never opens a dialog. Nora may
//! run editing commands only: not the commands that open, save or close a document or the
//! application, not those of the interface (`ui.`), and none that reaches a file of the computer by
//! a `path`. An action that fails leaves the document as it was, and what she changed can be undone a
//! whole action at a time.

use craft_nora::{
    AppKind, CommandCall, CommandPolicy, DocActionKind, DocumentInfo, HttpRequest, HttpResponse, Input, NoraSession, Output, PageContext, Selection,
    SessionConfig,
};
use craft_nora_ui::{NoraLabels, NoraPanel, NoraStyle, PanelAction, drive};
use egui::Ui;
use serde_json::{Value, json};

use crate::WordApp;
use crate::control::{ControlRequest, Outcome};

/// The most characters of the selection told to Nora with a message; she reads the document herself
/// when she needs more.
const MAX_SELECTION_CHARS: usize = 2_000;

/// The most characters of the document told to Nora when she reads it.
const MAX_OBSERVED_CHARS: usize = 50_000;

/// Parameter names that would reach a file of the computer. A command given one is refused.
const FILE_KEYS: &[&str] = &["path", "paths", "file", "files", "filename", "folder", "directory", "dir"];

/// What Nora may play in a WordCraft document: the editing commands. Not those that open, save or
/// close a document or the application (the default policy), and not those of the interface itself
/// (`ui.`: dialogs, settings, links), which belong to the user.
fn policy() -> CommandPolicy {
    CommandPolicy::default().deny_prefix("ui.")
}

/// What the pane keeps between frames: the conversation and the panel that draws it.
pub struct NoraState {
    session: NoraSession,
    panel: NoraPanel,
    /// How many undo steps each change Nora made took, the newest last, so that Undo takes back a whole
    /// action (a batch is several steps).
    undo_steps: Vec<usize>,
}

impl Default for NoraState {
    fn default() -> Self {
        let context = PageContext::editor(AppKind::Word, DocumentInfo::named(""));
        let config = SessionConfig { policy: policy(), ..SessionConfig::default() };
        NoraState { session: NoraSession::new(config, context), panel: NoraPanel::new(), undo_steps: Vec::new() }
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

/// Does what the conversation asks: a call goes to the host, a document action is played, a change
/// repaints. The results and the missing-connection answers come back as inputs and may ask for more,
/// so this goes round a few times at most.
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
                    let result = run_document(app, ctx, &call.kind);
                    next.extend(app.nora.session.handle(Input::DocumentResult { id: call.id, result }));
                }
                Output::Changed => ctx.request_repaint(),
            }
        }
        queue = next;
    }
}

/// Plays one action of Nora in the open document. A change is remembered by the number of undo steps
/// it took; a change that fails is taken back, so a failed action leaves the document as it was.
fn run_document(app: &mut WordApp, ctx: &egui::Context, kind: &DocActionKind) -> Result<Value, String> {
    // Typing is one undo step until something else happens: close the user's group first, so that what
    // Nora does steps on its own and neither joins the user's typing nor is joined by it.
    if kind.modifies() {
        app.session.close_typing();
    }
    let before = app.session.undo_depth();
    let result = apply(app, ctx, kind);
    if kind.modifies() {
        app.session.close_typing();
        match &result {
            Ok(_) => app.nora.undo_steps.push(app.session.undo_depth().saturating_sub(before)),
            Err(_) => {
                let added = app.session.undo_depth().saturating_sub(before);
                if added > 0 {
                    app.session.undo_steps(added);
                }
            }
        }
    }
    result
}

fn apply(app: &mut WordApp, ctx: &egui::Context, kind: &DocActionKind) -> Result<Value, String> {
    match kind {
        DocActionKind::Observe => observe(app),
        DocActionKind::Execute { command, params } => execute(app, command, params),
        DocActionKind::Batch { commands } => batch(app, commands),
        DocActionKind::SelectText { text, occurrence } => app.execute("select.text", json!({"text": text, "occurrence": occurrence})),
        DocActionKind::TypeText { text, paragraphs } => type_text(app, text, *paragraphs),
        DocActionKind::RenderPage { page, scale } => render_page(app, ctx, *page, *scale),
        DocActionKind::Save => app.execute("file.save", json!({})),
        DocActionKind::Undo => undo(app),
    }
}

/// One command of Nora's, unless it would reach a file of the computer.
fn execute(app: &mut WordApp, command: &str, params: &Value) -> Result<Value, String> {
    if let Some(key) = file_key(params) {
        return Err(format!("`{command}` is refused: `{key}` would reach a file of the computer"));
    }
    app.execute(command, params.clone())
}

/// The first parameter name, at any depth, that would reach a file.
fn file_key(params: &Value) -> Option<String> {
    match params {
        Value::Object(map) => map
            .iter()
            .find_map(|(key, value)| if FILE_KEYS.contains(&key.to_ascii_lowercase().as_str()) { Some(key.clone()) } else { file_key(value) }),
        Value::Array(list) => list.iter().find_map(file_key),
        _ => None,
    }
}

/// Several commands in order. The first that fails ends the batch with an error, and the caller takes
/// back the ones that ran.
fn batch(app: &mut WordApp, commands: &[CommandCall]) -> Result<Value, String> {
    let mut results = Vec::new();
    for call in commands {
        match execute(app, &call.command, &call.params) {
            Ok(value) => results.push(json!({"command": call.command, "ok": true, "result": value})),
            Err(error) => return Err(format!("`{}` failed: {error}", call.command)),
        }
    }
    Ok(Value::Array(results))
}

/// Types at the caret, replacing the selection. With `paragraphs` each line is a paragraph. Nora's
/// words are typed as she wrote them: Enter would otherwise correct the word before it, as it does
/// for the user's own typing, so AutoCorrect is off while she types.
fn type_text(app: &mut WordApp, text: &str, paragraphs: bool) -> Result<Value, String> {
    if !paragraphs {
        return app.execute("text.insert", json!({"text": text, "raw": true}));
    }
    let autocorrect = std::mem::replace(&mut app.session.prefs.autocorrect.enabled, false);
    let result = type_paragraphs(app, text);
    app.session.prefs.autocorrect.enabled = autocorrect;
    result
}

fn type_paragraphs(app: &mut WordApp, text: &str) -> Result<Value, String> {
    let mut last = Ok(Value::Null);
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            last = app.execute("text.newParagraph", json!({}));
        }
        if last.is_ok() && !line.is_empty() {
            last = app.execute("text.insert", json!({"text": line, "raw": true}));
        }
        if last.is_err() {
            break;
        }
    }
    last
}

/// What Nora learns when she reads the document: its name, its size, its text (cut at a length that
/// fits her) and the selection with its formatting.
fn observe(app: &mut WordApp) -> Result<Value, String> {
    let body = app.execute("document.text", json!({}))?;
    let text = body.get("text").and_then(Value::as_str).unwrap_or_default();
    let shown: String = text.chars().take(MAX_OBSERVED_CHARS).collect();
    let truncated = shown.len() < text.len();
    let selection = app.execute("document.selection", json!({}))?;
    let context = context_of(app);
    Ok(json!({
        "name": context.document.name,
        "pages": context.document.pages,
        "words": app.session.word_count(),
        "text": shown,
        "textTruncated": truncated,
        "selection": selection,
    }))
}

/// A page as a picture, to look at the result (the control channel's `ui.render`, a base64 PNG).
fn render_page(app: &mut WordApp, ctx: &egui::Context, page: u32, scale: f64) -> Result<Value, String> {
    let (request, _) = ControlRequest::new("ui.render", json!({"page": page, "scale": scale}));
    match crate::control::handle(app, ctx, &request) {
        Outcome::Done(reply) if reply.get("ok").and_then(Value::as_bool) == Some(true) => Ok(reply.get("result").cloned().unwrap_or(Value::Null)),
        Outcome::Done(reply) => Err(reply.get("error").and_then(Value::as_str).unwrap_or("the page could not be rendered").to_string()),
        Outcome::Screenshot { .. } => Err("the page could not be rendered".to_string()),
    }
}

/// Takes back the last change Nora made, whole.
fn undo(app: &mut WordApp) -> Result<Value, String> {
    let Some(steps) = app.nora.undo_steps.pop() else {
        return Err("there is nothing of Nora's to undo".to_string());
    };
    if steps == 0 {
        return Ok(json!({"undone": 0}));
    }
    app.execute("edit.undoTo", json!({"count": steps}))
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

    /// The words of the pane follow the interface language: French has them in Neoffice's own table.
    #[test]
    fn the_pane_speaks_french_when_the_interface_does() {
        let mut a = app(Services::default());
        a.run("ui.language", json!({"value": "fr"})).unwrap();
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.run_steps(4);
        assert!(h.query_by_label("Nora n'est pas encore disponible dans cette version").is_some());
        assert!(h.query_by_label("Nouvelle conversation").is_some());
        assert!(h.query_by_label("Demandez à Nora d'écrire, de réécrire, de traduire ou de mettre en forme votre document.").is_some());
        assert!(h.query_by_label("New conversation").is_none());
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

    /// What Nora asks for in the document is played, and she gets the result by callback.
    #[test]
    fn a_document_action_runs_and_nora_gets_the_observation() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Write a greeting");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        let first = take(&outbox);
        answer(&inbox, &first[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        let polls = take(&outbox);
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        answer(&inbox, actions, json!({"actions": [{"action": "doc_type_text", "text": "Bonjour tout le monde", "callback_id": "c1"}]}));
        h.run_steps(4);
        assert_eq!(h.state().session.word_count(), 4, "the text was typed in the document");
        let told = take(&outbox);
        let observation = told.iter().find(|r| r.path.ends_with("receive_observation")).expect("Nora gets an answer");
        assert_eq!(observation.body["callback_id"], "c1");
        assert_eq!(observation.body["observation"]["ok"], true);
    }

    /// A command the policy refuses never reaches the document, and Nora is told why.
    #[test]
    fn a_refused_command_never_runs() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Switch to dark");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        answer(&inbox, &take(&outbox)[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        let polls = take(&outbox);
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        answer(
            &inbox,
            actions,
            json!({"actions": [{"action": "doc_execute", "command": "ui.theme", "params": {"value": "dark"}, "callback_id": "c2"}]}),
        );
        h.run_steps(4);
        let told = take(&outbox);
        let observation = told.iter().find(|r| r.path.ends_with("receive_observation")).expect("Nora gets an answer");
        assert_eq!(observation.body["observation"]["ok"], false);
        assert!(observation.body["observation"]["error"].as_str().unwrap_or_default().contains("refused"));
        assert_ne!(h.state().ui.theme, crate::theme::Appearance::Dark, "the user's theme is theirs");
    }

    fn run(app: &mut WordApp, kind: DocActionKind) -> Result<Value, String> {
        run_document(app, &egui::Context::default(), &kind)
    }

    fn type_in(app: &mut WordApp, text: &str) {
        run(app, DocActionKind::TypeText { text: text.into(), paragraphs: false }).unwrap();
    }

    fn text_of(app: &mut WordApp) -> String {
        app.execute("document.text", json!({})).unwrap()["text"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn nora_reads_the_document_its_size_and_its_selection() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello big world");
        run(&mut a, DocActionKind::SelectText { text: "big".into(), occurrence: 1 }).unwrap();
        let seen = run(&mut a, DocActionKind::Observe).unwrap();
        assert_eq!(seen["text"], "Hello big world");
        assert_eq!(seen["words"], 3);
        assert_eq!(seen["pages"], 1);
        assert_eq!(seen["textTruncated"], false);
        assert_eq!(seen["selection"]["text"], "big");
        assert!(seen["name"].as_str().is_some_and(|name| !name.is_empty()));
    }

    #[test]
    fn what_nora_reads_is_cut_at_a_length_that_fits_her() {
        let mut a = app(Services::default());
        type_in(&mut a, &"é".repeat(MAX_OBSERVED_CHARS + 10));
        let seen = run(&mut a, DocActionKind::Observe).unwrap();
        assert_eq!(seen["text"].as_str().map(|text| text.chars().count()), Some(MAX_OBSERVED_CHARS));
        assert_eq!(seen["textTruncated"], true);
    }

    #[test]
    fn selecting_then_formatting_changes_the_selection_only() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello world");
        run(&mut a, DocActionKind::SelectText { text: "world".into(), occurrence: 1 }).unwrap();
        run(&mut a, DocActionKind::Execute { command: "format.bold".into(), params: json!({}) }).unwrap();
        assert_eq!(text_of(&mut a), "Hello world", "the words are the same");
        assert_eq!(run(&mut a, DocActionKind::Observe).unwrap()["selection"]["text"], "world");
    }

    #[test]
    fn paragraphs_split_the_lines_of_a_typed_text() {
        let mut a = app(Services::default());
        run(&mut a, DocActionKind::TypeText { text: "one\ntwo\n\nfour".into(), paragraphs: true }).unwrap();
        assert_eq!(text_of(&mut a), "one\ntwo\n\nfour");
    }

    #[test]
    fn undo_takes_back_a_whole_batch_and_only_what_nora_did() {
        let mut a = app(Services::default());
        type_in(&mut a, "mine ");
        a.nora.undo_steps.clear();
        let commands = vec![
            CommandCall { command: "text.insert".into(), params: json!({"text": "a"}) },
            CommandCall { command: "text.newParagraph".into(), params: json!({}) },
            CommandCall { command: "text.insert".into(), params: json!({"text": "b"}) },
        ];
        let results = run(&mut a, DocActionKind::Batch { commands }).unwrap();
        assert_eq!(results.as_array().map(Vec::len), Some(3));
        assert!(text_of(&mut a).ends_with("a\nb"));
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a).trim_end(), "mine", "the whole batch is gone and the user's own typing stays");
        assert!(run(&mut a, DocActionKind::Undo).is_err(), "nothing left of Nora's to undo");
    }

    #[test]
    fn a_failing_batch_leaves_the_document_as_it_was() {
        let mut a = app(Services::default());
        type_in(&mut a, "before");
        let remembered = a.nora.undo_steps.len();
        let commands = vec![
            CommandCall { command: "text.insert".into(), params: json!({"text": " and more"}) },
            CommandCall { command: "no.such.command".into(), params: json!({}) },
        ];
        let error = run(&mut a, DocActionKind::Batch { commands }).unwrap_err();
        assert!(error.contains("no.such.command"), "{error}");
        assert_eq!(text_of(&mut a), "before", "the command that ran was taken back");
        assert_eq!(a.nora.undo_steps.len(), remembered, "and nothing new is left to undo");
    }

    #[test]
    fn no_command_of_nora_reaches_a_file_of_the_computer() {
        let mut a = app(Services::default());
        for params in [json!({"path": "/etc/hosts"}), json!({"options": {"FileName": "x"}}), json!({"list": [{"directory": "/"}]})] {
            let error = run(&mut a, DocActionKind::Execute { command: "insert.picture".into(), params }).unwrap_err();
            assert!(error.contains("refused"), "{error}");
        }
        let batch = vec![CommandCall { command: "insert.picture".into(), params: json!({"path": "x.png"}) }];
        assert!(run(&mut a, DocActionKind::Batch { commands: batch }).is_err());
    }

    #[test]
    fn the_policy_keeps_the_interface_and_the_files_to_the_user() {
        use craft_nora::Decision;
        let policy = policy();
        assert_eq!(policy.check("format.bold"), Decision::Allow);
        assert_eq!(policy.check("ui.theme"), Decision::Deny);
        assert_eq!(policy.check("ui.dialog"), Decision::Deny);
        assert_eq!(policy.check("file.saveAs"), Decision::Deny);
        assert_eq!(policy.check("app.quit"), Decision::Deny);
    }

    #[test]
    fn a_page_can_be_rendered_for_nora_to_look_at() {
        let mut a = app(Services::default());
        type_in(&mut a, "Look at this");
        let seen = run(&mut a, DocActionKind::RenderPage { page: 1, scale: 0.5 }).unwrap();
        assert!(seen["png"].as_str().is_some_and(|png| png.starts_with("iVBOR")), "a base64 PNG");
        assert!(seen["width"].as_u64().is_some_and(|width| width > 0));
        assert!(run(&mut a, DocActionKind::RenderPage { page: 99, scale: 1.0 }).is_err());
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
