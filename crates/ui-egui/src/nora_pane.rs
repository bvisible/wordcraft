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
//! `render_page`, `save_document`) through [`WordApp::execute`]. Nora may run editing commands only
//! ([`refusal`] says which she may not): not those that open, save, close or replace a document or
//! the application; not those that are the person's (the interface, their view, panes and windows,
//! their undo history, their clipboard, their searches, Read Aloud, the tools for their next clicks,
//! macros, their settings, dictionary and fonts); none that would ask the person something (a
//! dialog, a file picker); none that reaches a file of the computer or carries a password. An action
//! that fails leaves the document as it was, its undo history included. What she changed can be
//! undone a whole action at a time, and only while the document is still as her change left it: the
//! pane's Undo never takes back the person's own work. An action that changed nothing says so
//! (`"changed": false`) and leaves nothing for the pane's Undo, and craft-nora (9d28340 on, which
//! `Cargo.toml` pins) offers no Undo for it either.
//!
//! While Nora works, the panel's Send button is a Stop button (craft-nora-ui's `PanelAction::Stop`, which
//! goes to the conversation like every other action). A failure of the connection to Nora is said in the
//! interface language ([`error_words`]).
//!
//! The cards of the conversation (a letter made, an e-mail draft, a letter to confirm as the company's template)
//! and the links of Nora's replies are shown by the panel; a page of the instance they lead to is opened by the host
//! ([`crate::Services::nora_open`]). Nora may print the document (`doc_print`: the print dialog opens, the person
//! confirms there) and ask for its PDF (`doc_export_pdf`: the PDF goes to an e-mail draft in the person's mailbox,
//! nothing is sent), as the person asked her in the pane.

mod guard;

use craft_nora::{
    AppKind, CommandCall, CommandPolicy, Decision, DocActionKind, DocumentInfo, HttpRequest, HttpResponse, Input, NoraError, NoraSession, Output,
    PageContext, Selection, SessionConfig,
};
use craft_nora_ui::{CorrectionWords, LauncherWords, NoraLabels, NoraPanel, NoraStyle, PaneMemory, PanelAction, ResumeWords, drive};
use egui::Ui;
use serde_json::{Value, json};
use wordcraft_doc::props::CharProps;
use wordcraft_engine::file_access::{self, AutomationRoots};
use wordcraft_engine::{EditSnapshot, ViewState};

use crate::WordApp;
use crate::control::{ControlRequest, Outcome};

/// The most characters of the selection told to Nora with a message; she reads the document herself
/// when she needs more.
const MAX_SELECTION_CHARS: usize = 2_000;

/// The most characters of the document told to Nora when she reads it.
const MAX_OBSERVED_CHARS: usize = 50_000;

/// Parameter names that reach a file of the computer when they hold its name, as text: a command
/// given one is refused. `path` also names a place in the document (a position's paragraph,
/// `{"path": [0, 2], "off": 3}`, or a place in an equation), a list of numbers that reaches no file;
/// `original` is the original document of a comparison; `import` the bibliography file Manage Sources reads.
/// `dir` is no folder: it is a direction (Text Direction, moving in an equation), and no command of
/// WordCraft that Nora may play reads a folder by `dir` (`file.listTemplates` is refused as a `file.` command).
const FILE_KEYS: &[&str] = &["path", "paths", "file", "files", "filename", "folder", "directory", "original", "import"];

/// The command families the conversation refuses before an action reaches the pane, besides the
/// default policy's (`file.`, `app.`), as `CommandPolicy` takes them (prefixes): the interface
/// (`ui.`); the person's view, panes and windows (the View tab); their undo history (`edit.undo`,
/// `edit.undoTo` and `edit.undoHistory`, `edit.redo`: they would take back the person's own work, and
/// the pane's Undo would then take theirs back as Nora's); and Read Aloud, which plays the document
/// through the computer's speech program.
const REFUSED_FAMILIES: &[&str] = &["ui.", "view.", "edit.undo", "edit.redo", "review.readAloud", "readAloud."];

/// The other commands that are the person's rather than the document's, refused whatever their
/// parameters; an entry ending in `*` stands for every command whose id it starts.
const THE_PERSON_S: &[&str] = &[
    // Their view and panes, set from another tab than View.
    "table.viewGridlines",
    "styles.pane",
    "styles.inspector",
    "smartArt.textPane",
    "edit.clipboardPane",
    "review.markup",
    "review.showMarkup",
    "review.trackingOptions",
    "review.proofing",
    "review.hideInk",
    "fields.toggleCode",
    "developer.designMode",
    "draw.replay",
    "outline.expand",
    "outline.collapse",
    "outline.toggle",
    "outline.showLevel*",
    "outline.showAll",
    "outline.firstLineOnly",
    "outline.showFormatting",
    "outline.close",
    // Repeat, which runs the person's last command again, out of this pane's sight.
    "edit.repeat",
    // Their clipboard and its pane, and the formatting the format painter holds for them.
    "edit.copy",
    "edit.cut",
    "edit.clipboardItems",
    "edit.pasteClipboardItem",
    "edit.pasteAllClipboard",
    "edit.deleteClipboardItem",
    "edit.clearClipboard",
    "edit.copyFormat",
    "edit.pasteFormat",
    "edit.formatPainter",
    // The Spike (v0.10.0), the person's other clipboard: it cuts into it and pastes all it holds.
    "edit.spike",
    "edit.insertSpike",
    // The tools that take their next clicks, and their pens.
    "draw.pen",
    "draw.pencil",
    "draw.highlighter",
    "draw.select",
    "draw.addPen",
    "draw.removePen",
    "draw.inkToShape",
    // Macros, whose steps run out of this pane's sight (a step that saves a file, one that undoes the
    // person's work).
    "tools.macros",
    "tools.recordMacro",
    // Their settings, their dictionary (kept in a file of the computer) and the fonts they added (kept
    // with the app).
    "text.overtype",
    "tools.autocorrect",
    "tools.customizeKeyboard",
    "tools.customizeRibbon",
    "review.addToDictionary",
    "review.ignoreAll",
    "font.add",
    "font.remove",
    "font.loadSystem",
];

/// What Nora may play in a WordCraft document, as far as the conversation decides it: everything but
/// the default policy's files and application and the [`REFUSED_FAMILIES`]. The pane refuses the rest
/// of what is the person's itself ([`refusal`]).
fn policy() -> CommandPolicy {
    REFUSED_FAMILIES.iter().fold(CommandPolicy::default(), |policy, family| policy.deny_prefix(*family))
}

/// What the pane keeps between frames: the conversation, the panel that draws it, and what Nora
/// changed.
pub struct NoraState {
    session: NoraSession,
    panel: NoraPanel,
    /// Nora's changes that the pane's Undo can take back, the newest last.
    changes: Vec<Change>,
    /// While the pane plays a command of Nora's: what the command asked of the interface (a dialog, a
    /// file picker, a pane's search box). [`WordApp`] keeps its requests here instead of acting on them,
    /// and the pane refuses the command ([`execute`]).
    pub(crate) asked: Option<Vec<Value>>,
}

/// One change of Nora's: the document it was made in, the undo steps it took there, and the
/// document's revision when it began and once it was made.
struct Change {
    document: u64,
    steps: usize,
    /// The revision ([`wordcraft_engine::Session::rev`], which every edit, undo and redo moves on)
    /// when the change began: the change made just before ended there when nothing came between them.
    before: u64,
    /// The revision once the change was made (or once the pane took back the change made right after
    /// it, or an action that failed right after it): the pane's Undo takes the change back only while
    /// the document is still at it. A count of steps alone can't tell: the person's own Undo puts back
    /// the very document one of Nora's steps restores, and the step of their next edit restores it too.
    after: u64,
}

impl Default for NoraState {
    fn default() -> Self {
        let context = PageContext::editor(AppKind::Word, DocumentInfo::named(""));
        let config = SessionConfig { policy: policy(), ..SessionConfig::default() };
        NoraState {
            session: NoraSession::new(config, context),
            panel: NoraPanel::new().with_error_words(error_words),
            changes: Vec::new(),
            asked: None,
        }
    }
}

/// Draws the pane's content (inside the pane's frame) and moves the conversation along.
/// The name the pane's state is remembered under in the browser (craft-nora-ui `PaneMemory`).
const MEMORY: &str = "wordcraft";

/// Turns the pane on where the page offers Nora, unless the person closed it on an earlier visit (remembered in the
/// browser, open when nothing is); off where Nora isn't offered.
pub fn open_as_remembered(app: &mut WordApp, offered: bool) -> Result<Value, String> {
    app.execute("ui.nora", json!({"value": offered && PaneMemory::new(MEMORY).open()}))
}

/// Keeps whether the person left the pane open (their close or the rail's click), in the browser.
pub(crate) fn remember(open: bool) {
    PaneMemory::new(MEMORY).remember(open);
}

/// Nora's button in the title bar, at the right beside the search box, whatever tab of the ribbon is in view, where the
/// page offers Nora (craft-nora-ui `launcher::button`, NeoMail's « Ask Nora »): a click opens or closes the pane on the
/// same conversation, and the person's choice is remembered in the browser. Lit while the pane is open.
pub fn top_button(app: &mut WordApp, ui: &mut Ui) {
    if app.services.nora_http.is_none() {
        return;
    }
    let words = LauncherWords { label: tl!("Ask Nora").to_string(), tip: tl!("Nora, the Neoffice assistant").to_string() };
    let open = app.ui.nora_pane;
    ui.add_space(8.0);
    let response = craft_nora_ui::launcher::button(ui, &words, open);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, &words.label));
    if response.clicked() && app.run("ui.nora", json!({"value": !open})).is_ok() {
        remember(!open);
    }
}

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    // The frame's tick first, then the answers (craft-libs ed1cb40): after a gap without frames (a tab in the
    // background) the session knows it before it reads an answer that waited in the page, whose action it then refuses
    // rather than plays late; from 1f01bcc on, the tick back gives nothing up on the clock before those answers.
    let mut outputs = drive(ui.ctx(), &mut app.nora.session);
    outputs.extend(deliver_answers(app));
    app.nora.panel.labels = labels();
    app.nora.panel.resume_words = resume_words();
    app.nora.panel.correction_words = correction_words();
    app.nora.panel.style = style(ui);
    app.nora.panel.offline = app.services.nora_http.is_none().then(|| tl!("Nora isn't available in this version yet").to_string());
    app.nora.panel.open_links = app.services.nora_open.is_some();
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
                Output::Open(path) => {
                    if let Some(open) = &app.services.nora_open {
                        open(&path);
                    }
                }
            }
        }
        queue = next;
    }
}

/// Plays one action of Nora in the open document. One that can change it (a command, a batch, typing)
/// is played against a snapshot of the document, its selection, its undo history and the person's
/// view: when the action fails, all of them go back to the snapshot, so a failed action leaves the
/// document as it was. One that changed something is remembered for the pane's Undo, with the undo
/// steps it took and the revision it left; one that changed nothing (no undo step, the revision it
/// started from) answers with `"changed": false`, for which craft-nora (9d28340 on) leaves nothing to
/// undo.
fn run_document(app: &mut WordApp, ctx: &egui::Context, kind: &DocActionKind) -> Result<Value, String> {
    if !kind.modifies() {
        return apply(app, ctx, kind);
    }
    // Typing is one undo step until something else happens: close the user's group first, so that what
    // Nora does steps on its own and neither joins the user's typing nor is joined by it.
    app.session.close_typing();
    let before = Before::take(app);
    let result = apply(app, ctx, kind);
    app.session.close_typing();
    match result {
        Ok(value) => {
            let steps = usize::try_from(app.session.undo_mark().saturating_sub(before.pushed)).unwrap_or(usize::MAX);
            let after = app.session.rev();
            if steps == 0 && after == before.rev && app.session.document_id() == before.document {
                return Ok(unchanged(value));
            }
            // What the action really wrote: a link or a field that leads outside the document takes it all back.
            let found = guard::added(&before.written, &app.session.doc);
            if !found.is_empty() {
                before.restore(app);
                return Err(guard::refusal(&what_ran(kind), &found));
            }
            app.nora.changes.push(Change { document: before.document, steps, before: before.rev, after });
            Ok(value)
        }
        Err(error) => {
            before.restore(app);
            Err(error)
        }
    }
}

/// How Nora is told which of her actions was taken back.
fn what_ran(kind: &DocActionKind) -> String {
    match kind {
        DocActionKind::Execute { command, .. } => format!("`{command}`"),
        DocActionKind::Batch { .. } => "The batch".to_string(),
        DocActionKind::TypeText { .. } => "The typed text".to_string(),
        _ => "The action".to_string(),
    }
}

/// The answer of an action that changed nothing: its result with `"changed": false` (craft-nora, from
/// 9d28340 on, then leaves nothing to undo for it). An object carries the mark itself; any other result, a batch's
/// list for one, goes under `result`.
fn unchanged(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("changed".into(), Value::Bool(false));
            Value::Object(map)
        }
        other => json!({"result": other, "changed": false}),
    }
}

/// The document as it was before an action of Nora's, to go back to when the action fails.
struct Before {
    document: u64,
    rev: u64,
    /// [`wordcraft_engine::Session::undo_mark`]: what the action adds to it is the undo steps it took,
    /// also at the undo limit.
    pushed: u64,
    /// Every undo step is kept: a batch may push many out of a full history.
    snapshot: EditSnapshot,
    view: ViewState,
    /// The formatting waiting at the caret for the person's next typing (Bold with nothing selected).
    pending: Option<CharProps>,
    /// The document's stories as they were, for what the action writes ([`guard`]).
    written: guard::Before,
}

impl Before {
    fn take(app: &WordApp) -> Before {
        let s = &app.session;
        Before {
            document: s.document_id(),
            rev: s.rev(),
            pushed: s.undo_mark(),
            snapshot: s.edit_snapshot_whole(),
            view: s.view.clone(),
            pending: s.pending.clone(),
            written: guard::Before::of(&s.doc),
        }
    }

    /// Puts the document back as it was, with its selection, its undo history, the person's view and
    /// the formatting waiting at their caret: the person's Redo comes back too, and no step of the failed
    /// action is left to undo or redo. (No command Nora may play takes undo steps off or replaces the
    /// document, so the snapshot comes back exactly.) A change of Nora's that had left the document so is
    /// the newest thing in it again.
    fn restore(self, app: &mut WordApp) {
        if app.session.document_id() != self.document {
            return;
        }
        app.session.restore(self.snapshot);
        app.session.view = self.view;
        app.session.pending = self.pending;
        let now = app.session.rev();
        if let Some(previous) = app.nora.changes.last_mut()
            && previous.document == self.document
            && previous.after == self.rev
        {
            previous.after = now;
        }
    }
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
        DocActionKind::Print => print(app),
        DocActionKind::ExportPdf => export_pdf(app),
        DocActionKind::ExportFile => export_file(app),
    }
}

/// One command of Nora's, unless [`refusal`] says it is not hers to play. It runs with no file of the
/// computer within reach (automation roots that grant nothing are in force, whatever its parameters
/// are called), and what it asks of the interface is kept instead of done: a command that would open
/// a dialog or a file picker, or put the person's typing in a pane's search box, needs the person, and
/// is refused (the caller takes back what it did). It searches with Find's defaults, and the person's
/// Find is as they left it afterwards.
fn execute(app: &mut WordApp, command: &str, params: &Value) -> Result<Value, String> {
    if let Some(why) = refusal(command, params) {
        return Err(why);
    }
    app.nora.asked = Some(Vec::new());
    // The person's Find (what they searched for, their Replace with text, their options) stays theirs: Nora's command
    // starts from Find's defaults (the person's regular expressions or Match case don't change what her text means),
    // and theirs is put back after it.
    let find = std::mem::take(&mut app.session.find);
    let no_files = AutomationRoots::none();
    let result = file_access::confine(Some(&no_files), || app.execute(command, params.clone()));
    app.session.find = find;
    let asked = app.nora.asked.take().unwrap_or_default();
    if let Some(what) = asked.iter().find_map(needs_the_person) {
        return Err(format!("`{command}` is refused: it needs the person, as it would {what}"));
    }
    result
}

/// Why Nora may not play `id` with `params`, when she may not: a command that is the person's
/// ([`is_the_person_s`], [`the_person_s_with`]), one that would replace the document open, and one given
/// a parameter that names a file of the computer or holds a password ([`forbidden_key`]). The
/// conversation refuses the [`REFUSED_FAMILIES`] already; this is asked of every command the pane
/// plays, one of a batch too.
fn refusal(id: &str, params: &Value) -> Option<String> {
    if is_the_person_s(id) {
        return Some(format!("`{id}` is refused: it is the person's, not the document's"));
    }
    if let Some(why) = the_person_s_with(id, params) {
        return Some(format!("`{id}` is refused: {why}"));
    }
    if replaces_the_document(id, params) {
        return Some(format!("`{id}` is refused: it would replace the document open, the one Nora edits"));
    }
    forbidden_key(params).map(|(key, why)| format!("`{id}` is refused: `{key}` {why}"))
}

/// Whether `id` would replace the document open, or make it another one: the commands that open, make or
/// close one (New, Open, Close, the mail merge's new documents: `switches_document`), and a comparison
/// shown in a new document, which turns the one open into an untitled one, its file forgotten.
fn replaces_the_document(id: &str, params: &Value) -> bool {
    let compared_elsewhere = || params.get("showIn").and_then(Value::as_str).unwrap_or("original") != "original";
    crate::switches_document(id, params) || (matches!(id, "review.compare" | "review.combine") && compared_elsewhere())
}

/// Whether `id` is the person's whatever its parameters: of the default policy's families or of the
/// [`REFUSED_FAMILIES`], or one of [`THE_PERSON_S`].
fn is_the_person_s(id: &str) -> bool {
    policy().check(id) == Decision::Deny
        || THE_PERSON_S.iter().any(|entry| match entry.strip_suffix('*') {
            Some(prefix) => id.starts_with(prefix),
            None => id == *entry,
        })
}

/// What makes `id` the person's with these parameters, when they do. Each test reads the parameters as
/// the command itself does: a parameter that is there (even `null`), one that is text, one that is a
/// boolean.
fn the_person_s_with(id: &str, params: &Value) -> Option<&'static str> {
    let present = |key: &str| params.get(key).is_some();
    let given = |key: &str| params.get(key).is_some_and(|value| !value.is_null());
    let text = |key: &str| params.get(key).and_then(Value::as_str).is_some();
    let words = |key: &str| params.get(key).and_then(Value::as_str).is_some_and(|text| !text.is_empty());
    if pastes_the_clipboard(id, params) {
        return Some("it would paste the person's clipboard: give it what to paste, as text");
    }
    match id {
        // Without a place to work on, these pick up a tool for the person's next clicks.
        "draw.eraser" if !["page", "x", "y"].into_iter().any(present) => Some("without a point, it hands the person the eraser"),
        "draw.lasso" if params.get("clear").and_then(Value::as_bool) != Some(true) && !present("points") => {
            Some("without points, it hands the person the lasso")
        }
        "table.draw" | "table.eraser" if !present("page") => Some("without a page, it hands the person the table pen"),
        // Without a style, Apply Styles only shows the person's Styles pane.
        "styles.apply" if !present("style") => Some("without a style, it only shows the person's Styles pane"),
        // These parameters are the person's settings.
        "review.wordCount" if given("includeTextBoxes") => Some("`includeTextBoxes` is the person's setting for counting words"),
        "review.language" if given("detect") => Some("`detect` is the person's setting"),
        // `default: true` also saves the language as the person's own for their future blank documents (a preference kept
        // outside the document); the language of the text itself (`default` absent or false) stays Nora's.
        "review.language" if params.get("default").and_then(Value::as_bool) == Some(true) => {
            Some("`default` would also save the person's language for their future documents, outside this one")
        }
        // Without the template's bytes, the template the document names is read from the computer.
        "tools.updateStyles" if !text("data") => Some("without `data`, it would read the attached template, a file of the computer"),
        // Replace with `^c` is the person's clipboard; without `with` or `text`, Replace uses what they typed last
        // in Find and Replace.
        "edit.replace" | "edit.replaceAll" if params.get("with").and_then(Value::as_str).is_some_and(|with| with.contains("^c")) => {
            Some("`^c` in `with` is the person's clipboard")
        }
        "edit.replace" | "edit.replaceAll" if !text("with") || !words("text") => {
            Some("without `text` and `with`, it would replace what the person last searched for, or with what they last typed")
        }
        // A search without its text searches for what the person searched for last, and selects it.
        "edit.find" | "edit.findNext" | "edit.findPrevious" | "edit.advancedFind" if !words("text") => {
            Some("without `text`, it would search for what the person searched for last")
        }
        // Reading Highlight marks the matches in the person's view.
        "edit.advancedFind" if present("highlight") => Some("`highlight` changes the person's view"),
        _ => None,
    }
}

/// Whether a paste takes what it pastes from the person's clipboard, as the paste commands read their
/// parameters: unless Nora gives it, as text that is not empty, in a parameter the command pastes from (Paste and Merge
/// Formatting: `text`, `html` or `rtf`; Keep Text Only: `text`; Paste Special: the chosen format's own,
/// `text` standing in for HTML and RTF, and `picture` for a picture, which the command would otherwise
/// answer with what the person's clipboard holds). Paste Special's Keep Source Formatting is the person's
/// own copy whatever is given, and without a format it lists what their clipboard holds.
fn pastes_the_clipboard(id: &str, params: &Value) -> bool {
    let text = |key: &str| params.get(key).and_then(Value::as_str).is_some_and(|text| !text.is_empty());
    match id {
        "edit.paste" | "edit.pasteMerge" => !(text("text") || text("html") || text("rtf")),
        "edit.pasteText" => !text("text"),
        "edit.pasteSpecial" => match params.get("as").and_then(Value::as_str).map(str::to_ascii_lowercase).as_deref() {
            Some("text" | "unformatted" | "plain") => !text("text"),
            Some("html") => !(text("html") || text("text")),
            Some("rtf") => !(text("rtf") || text("text")),
            Some("picture" | "png" | "image") => !text("picture"),
            Some("formatted" | "keep" | "wordcraft") | None => true,
            // An unknown format: the command answers with an error that says nothing of the clipboard.
            Some(_) => false,
        },
        _ => false,
    }
}

/// The first parameter, at any depth, that Nora may not give, and why: one of [`FILE_KEYS`] holding
/// text (a file's name), or a password (`password`, `newPassword`…), which would lock the person out of
/// their own document with a password they never chose, or try one they gave nobody.
fn forbidden_key(params: &Value) -> Option<(String, &'static str)> {
    match params {
        Value::Object(map) => map.iter().find_map(|(key, value)| {
            let lowered = key.to_ascii_lowercase();
            if FILE_KEYS.contains(&lowered.as_str()) && holds_text(value) {
                Some((key.clone(), "would reach a file of the computer"))
            } else if lowered.contains("password") {
                Some((key.clone(), "is a password, and passwords are the person's"))
            } else {
                forbidden_key(value)
            }
        }),
        Value::Array(list) => list.iter().find_map(forbidden_key),
        _ => None,
    }
}

/// Whether `value` holds text anywhere: a file's name does, a place in the document (numbers) doesn't.
fn holds_text(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Array(list) => list.iter().any(holds_text),
        Value::Object(map) => map.values().any(holds_text),
        _ => false,
    }
}

/// What a request a command made of the interface would have done, when it needs the person: anything
/// but scrolling to the caret.
fn needs_the_person(request: &Value) -> Option<String> {
    if request.get("scrollTo").is_some() {
        return None;
    }
    Some(match (request.get("open").and_then(Value::as_str), request.get("focus").and_then(Value::as_str)) {
        (Some(what), _) => format!("open `{what}` in front of the person (a dialog, a page of the File menu or a file picker)"),
        (None, Some(what)) => format!("put the person's typing in `{what}`"),
        (None, None) => format!("ask the interface for {request}"),
    })
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

/// Takes back the last change Nora made, whole, while the document is still as that change left it
/// (its revision): when the person changed the document since, or undid the change, the pane leaves
/// their work alone and says to use the editor's Undo. Refused, it forgets every change of Nora's, as
/// craft-nora does from 9d28340 on: the older ones lie under the person's edit too. Done, the change made just
/// before, when nothing came between the two, is the newest thing in the document again.
fn undo(app: &mut WordApp) -> Result<Value, String> {
    let Some(change) = app.nora.changes.pop() else {
        return Err(tl!("There is nothing of Nora's to undo.").to_string());
    };
    if app.session.document_id() != change.document || app.session.rev() != change.after {
        app.nora.changes.clear();
        return Err(tl!("You changed the document after Nora did: use the editor's Undo instead.").to_string());
    }
    if change.steps > 0
        && let Err(error) = app.execute("edit.undoTo", json!({"count": change.steps}))
    {
        app.nora.changes.clear();
        return Err(error);
    }
    let now = app.session.rev();
    if let Some(previous) = app.nora.changes.last_mut()
        && previous.document == change.document
        && previous.after == change.before
    {
        previous.after = now;
    }
    Ok(json!({"undone": change.steps}))
}

/// The print dialog on the document, as File › Print makes it (`printing::submit`): the person chooses and confirms
/// there, nothing is printed before. Where the editor has printers of its own (`print_native`, which prints without a
/// dialog once a printer is chosen), its Print page opens instead: the person picks the printer and prints there.
fn print(app: &mut WordApp) -> Result<Value, String> {
    if app.services.print_native.is_some() {
        app.run("file.print", json!({}))?;
        return Ok(json!({"dialog": true, "page": "print", "changed": false}));
    }
    crate::printing::submit(app, &json!({}))?;
    Ok(json!({"dialog": true, "changed": false}))
}

/// What Nora is told when she asks for the PDF of a document opened with its password: it would leave in clear.
const PROTECTED_STAYS: &str = "this document was opened with its password: its PDF is never handed over in clear for an e-mail, the person \
joins the document from NeoMail";

/// The document's PDF, as File › Export › PDF makes it, handed to Nora for an e-mail draft (craft-nora's
/// `pdf_result`: in base64, refused past its size). Never the PDF of a document that has a password.
fn export_pdf(app: &mut WordApp) -> Result<Value, String> {
    if app.session.password.is_some() {
        return Err(PROTECTED_STAYS.to_string());
    }
    let name = format!("{}.pdf", app.title_stem());
    let opts = wordcraft_engine::cmd::file::export_options(&app.session, &json!({}));
    let bytes = wordcraft_engine::io::save_bytes_with(&name, &app.session.doc, None, &opts)?;
    craft_nora::protocol::pdf_result(&name, &bytes)
}

/// The document in its own format (the page's file's: `.docx`…), as it is now with what was not saved yet, handed to
/// Nora for an e-mail draft (craft-nora's `file_result`). A document that has a password goes encrypted with it, as a
/// save writes it. Nothing is saved.
fn export_file(app: &mut WordApp) -> Result<Value, String> {
    let name = app.default_save_name();
    let opts = wordcraft_engine::cmd::file::export_options(&app.session, &json!({}));
    let password = app.session.password.as_ref().map(wordcraft_engine::Password::as_str);
    let bytes = wordcraft_engine::io::save_bytes_with(&name, &app.session.doc, password, &opts)?;
    craft_nora::protocol::file_result(&name, &bytes)
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

/// The file the hosting page named the open document (`Services::nora_file`): the page's last file, when the
/// document is that file (its name), and only an id that reads as one ([`DocumentInfo::file_id`]). The server never
/// believes it as it comes, it reads the file again under the person's rights. A page that names none names none.
fn named_file(app: &WordApp, name: &str) -> Option<String> {
    let file = app.services.nora_file.as_ref()?.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    let (sent, id) = file?;
    let sent = std::path::Path::new(&sent).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(sent);
    if sent != name {
        return None;
    }
    DocumentInfo::file_id(&id?)
}

/// Where the user is: the editor, the document's name and page count, and the selected text.
fn context_of(app: &mut WordApp) -> PageContext {
    let name = app.session.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| app.title_stem());
    let pages = u32::try_from(app.session.layout().pages.len()).ok();
    let selected = app.session.selected_text();
    let selection = (!selected.is_empty()).then(|| Selection { text: selected.chars().take(MAX_SELECTION_CHARS).collect() });
    let id = named_file(app, &name);
    PageContext::editor(AppKind::Word, DocumentInfo { name, id, pages, selection })
}

/// The pane's bubbles in WordCraft's own colors, light or dark.
fn style(ui: &Ui) -> NoraStyle {
    let t = crate::theme::Tokens::get(ui.ctx());
    NoraStyle {
        user_fill: Some(t.accent),
        user_text: Some(t.on_accent),
        nora_fill: Some(t.hover),
        nora_text: Some(t.text),
        // The pane's bold words in the app's own bold face.
        bold_family: Some("semibold"),
        ..NoraStyle::default()
    }
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
        stop: tl!("Stop").to_string(),
        working: tl!("Nora is working…").to_string(),
        stopped: tl!("Stopped.").to_string(),
        empty: tl!("Ask Nora to write, rewrite, translate or format your document.").to_string(),
        confirm_heading: tl!("Nora wants to:").to_string(),
        allow: tl!("Allow").to_string(),
        refuse: tl!("Refuse").to_string(),
        open: tl!("Open").to_string(),
        describe,
    }
}

/// What the pane says under a request lost to a tab in the background, and its button, in the interface language.
fn resume_words() -> ResumeWords {
    ResumeWords { said: tl!("This tab was in the background: Nora could not act in the document.").to_string(), resume: tl!("Resume").to_string() }
}

/// What the pane says of a reply whose turn could not act in the document in time, in the interface language.
fn correction_words() -> CorrectionWords {
    CorrectionWords {
        nothing_done: tl!("Nothing was done in the document: Nora could not act in it in time. Ask again if you still need it.").to_string(),
        partly_done: tl!("Not all of this was done: part of it could not run in the document in time.").to_string(),
    }
}

/// A failure of the connection to Nora in the interface language (the panel shows the editor's own
/// sentences, an action it refused, as they come). The server's own words, when it gave some, are shown as
/// they are: the server writes them for the person.
fn error_words(failure: &NoraError) -> String {
    match failure {
        NoraError::Timeout => tl!("Nora did not answer in time.").to_string(),
        NoraError::Unreachable(_) => tl!("Nora could not be reached. Check the connection and try again.").to_string(),
        NoraError::Http(401 | 403) => tl!("Your session has expired. Sign in again.").to_string(),
        NoraError::Http(status) => {
            crate::i18n::fmt(tl!("The server could not answer (error {status}). Try again in a moment."), &[("status", &status.to_string())])
        }
        NoraError::Server { message, .. } => message.clone(),
        NoraError::Malformed(_) => tl!("Nora's answer could not be read.").to_string(),
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
        // Never waiting for a yes (the print dialog is the person's; the PDF goes to a draft): craft-nora's words.
        DocActionKind::Print | DocActionKind::ExportPdf | DocActionKind::ExportFile => kind.summary(),
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

    /// Nora's button in the title bar closes the pane and opens it again, on the same conversation; it is there only where
    /// Nora is offered. Outside a browser nothing is remembered: the pane opens as before.
    #[test]
    fn nora_s_button_in_the_title_bar_closes_and_opens_the_pane() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        assert_eq!(open_as_remembered(&mut a, true).unwrap(), json!({"nora": true}));
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Bonjour");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        let first = take(&outbox);
        answer(&inbox, &first[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        assert_eq!(h.state().nora.session.thread_id(), Some("t1"));
        let button = h.get_by_label("Ask Nora").rect();
        assert!(button.max.y < 60.0, "in the title bar (the test window keeps a margin of its own): {button:?}");
        h.get_by_label("Ask Nora").click();
        h.run_steps(4);
        assert!(!h.state().ui.nora_pane, "closed");
        h.get_by_label("Ask Nora").click();
        h.run_steps(4);
        assert!(h.state().ui.nora_pane, "open again");
        assert_eq!(h.state().nora.session.thread_id(), Some("t1"), "on the same conversation");
        // Where Nora isn't offered: off, and no button.
        let mut alone = app(Services::default());
        assert_eq!(open_as_remembered(&mut alone, false).unwrap(), json!({"nora": false}));
        let mut h = harness(alone);
        h.run_steps(4);
        assert!(h.query_by_label("Ask Nora").is_none());
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

    /// A tab left right after a send draws no frame for minutes: back, the host ticks before it hands over the answers
    /// that waited in the page, and nothing is given up on the clock at the tick back (craft-libs 1f01bcc); what Nora
    /// queued while the page ran nothing is refused, never played late, and the pane offers the request again, sent on
    /// the click only.
    #[test]
    fn back_from_a_tab_in_the_background_nothing_is_given_up_and_the_request_can_be_resumed() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.input_mut().time = Some(1.0);
        h.state_mut().nora.panel.set_draft("Write a greeting");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(2);
        let first = take(&outbox);
        let start = first.iter().find(|r| r.path.ends_with("start_or_resume_thread")).expect("the message leaves").clone();
        // The tab is left at once: eight minutes without a frame, the send's answer waiting in the page.
        answer(&inbox, &start, json!({"thread_id": "t1", "sent_at": 1000.0}));
        h.input_mut().time = Some(481.0);
        h.run_steps(2);
        assert!(h.state().nora.session.failure().is_none(), "nothing is given up at the tick back");
        let polls = take(&outbox);
        let reads = polls.iter().find(|r| r.path.ends_with("get_messages")).expect("the replies are read");
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        // What Nora queued while the page ran nothing waited eight minutes, by the server's clock.
        let late = json!({"action": "doc_type_text", "text": "Bonjour", "callback_id": "c1"});
        answer(&inbox, actions, json!({"actions": [late], "queued_at": [1002.0], "now": 1482.0}));
        let reply = json!([{"role": "assistant", "text": "Rien n'a été modifié.", "ts": 1483.0}]);
        answer(&inbox, reads, json!({"messages": reply}));
        h.run_steps(2);
        // The turn ends with Nora's reply; reads still on their way are answered alike.
        for at in [484.0, 490.0, 496.0] {
            for request in take(&outbox) {
                if request.path.ends_with("get_messages") {
                    answer(&inbox, &request, json!({"messages": reply}));
                } else if request.path.ends_with("get_browser_actions") {
                    answer(&inbox, &request, json!({"actions": []}));
                }
            }
            h.input_mut().time = Some(at);
            h.run_steps(2);
        }
        assert_eq!(h.state().session.word_count(), 0, "the late action is never played");
        assert_eq!(h.state().nora.session.resumable(), Some("Write a greeting"));
        assert!(h.query_by_label("This tab was in the background: Nora could not act in the document.").is_some());
        take(&outbox);
        h.get_by_label("Resume").click();
        h.run_steps(2);
        let again = take(&outbox);
        let post = again.iter().find(|r| r.path.ends_with("post_message")).expect("the same request leaves again");
        assert_eq!(post.body["message"], "Write a greeting");
    }

    // //// Neoffice — print, the PDF for an e-mail draft, and the pages a card opens (step 7 of « Nora partout »).

    /// « Imprime-la »: the print dialog of the host opens on the document's PDF, as File › Print makes it.
    #[test]
    fn nora_prints_through_the_print_dialog_only() {
        let printed: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        let seen = printed.clone();
        let services = Services {
            print: Some(Box::new(move |bytes: &[u8]| {
                seen.lock().unwrap_or_else(|e| e.into_inner()).push(bytes.to_vec());
                Ok(())
            })),
            ..Services::default()
        };
        let mut a = app(services);
        type_in(&mut a, "Lettre à imprimer");
        let answer = run(&mut a, DocActionKind::Print).unwrap();
        assert_eq!(answer, json!({"dialog": true, "changed": false}));
        let printed = printed.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(printed.len(), 1);
        assert!(printed[0].starts_with(b"%PDF-"), "the dialog gets the document's PDF");
        // Without a print dialog, Nora is told so: nothing is printed another way.
        let mut b = app(Services::default());
        assert!(run(&mut b, DocActionKind::Print).is_err());
    }

    /// « Envoie-la par e-mail »: the document's PDF, in base64, for the draft the server writes.
    #[test]
    fn nora_gets_the_document_s_pdf_for_an_e_mail_draft() {
        let mut a = app(Services::default());
        type_in(&mut a, "Lettre à envoyer");
        let answer = run(&mut a, DocActionKind::ExportPdf).unwrap();
        assert_eq!(answer["name"], "Document1.pdf");
        let data = answer["data"].as_str().unwrap_or_default();
        assert!(data.starts_with("JVBERi0"), "a PDF, in base64");
        let size = usize::try_from(answer["size"].as_u64().unwrap_or_default()).unwrap_or_default();
        assert!(size > 0 && data.len() == size.div_ceil(3) * 4, "the size is the PDF's, the data its base64");
        assert_eq!(text_of(&mut a), "Lettre à envoyer", "the document is left as it was");
    }

    /// « Envoie-le par e-mail »: the document in its own format, as it is now, for the draft the server writes; nothing
    /// is saved.
    #[test]
    fn nora_gets_the_document_in_its_format_for_an_e_mail_draft() {
        let mut a = app(Services::default());
        type_in(&mut a, "Rapport à envoyer");
        let answer = run(&mut a, DocActionKind::ExportFile).unwrap();
        assert_eq!(answer["name"], "Document1.docx");
        let data = answer["data"].as_str().unwrap_or_default();
        assert!(data.starts_with("UEsDB"), "a Word package (a zip), in base64");
        assert_eq!(text_of(&mut a), "Rapport à envoyer", "the document is left as it was");
    }

    /// A card's Open goes to the host as a path of the instance; without a host that opens pages the pane offers none.
    #[test]
    fn a_page_of_the_instance_is_opened_by_the_host() {
        let opened: Arc<Mutex<Vec<String>>> = Arc::default();
        let seen = opened.clone();
        let (mut services, _outbox, _inbox) = connected();
        services.nora_open = Some(Box::new(move |path: &str| seen.lock().unwrap_or_else(|e| e.into_inner()).push(path.to_string())));
        let mut a = app(services);
        let outputs = a.nora.session.handle(Input::OpenLink("/drive/f/abc".into()));
        let ctx = egui::Context::default();
        perform(&mut a, outputs, &ctx);
        let outputs = a.nora.session.handle(Input::OpenLink("https://elsewhere.example/x".into()));
        perform(&mut a, outputs, &ctx);
        assert_eq!(*opened.lock().unwrap_or_else(|e| e.into_inner()), vec!["/drive/f/abc".to_string()]);
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
        a.nora.changes.clear();
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
        let remembered = a.nora.changes.len();
        let commands = vec![
            CommandCall { command: "text.insert".into(), params: json!({"text": " and more"}) },
            CommandCall { command: "no.such.command".into(), params: json!({}) },
        ];
        let error = run(&mut a, DocActionKind::Batch { commands }).unwrap_err();
        assert!(error.contains("no.such.command"), "{error}");
        assert_eq!(text_of(&mut a), "before", "the command that ran was taken back");
        assert_eq!(a.nora.changes.len(), remembered, "and nothing new is left to undo");
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
        // The person's view and windows, their undo history and Read Aloud: refused by the conversation already.
        for id in [
            "view.darkMode",
            "view.zoom",
            "view.newWindow",
            "view.split",
            "edit.undo",
            "edit.undoTo",
            "edit.redo",
            "review.readAloud",
            "readAloud.playPause",
        ] {
            assert_eq!(policy.check(id), Decision::Deny, "{id}");
        }
        for id in ["text.insert", "edit.paste", "edit.replaceAll", "review.newComment", "table.insertRowBelow"] {
            assert_eq!(policy.check(id), Decision::Allow, "{id}");
        }
    }

    /// Read Aloud plays the document through the computer's speech program: its commands are the person's, refused
    /// by the conversation before they reach the pane. (Never run here: on a desktop they would speak.)
    #[test]
    fn nora_never_plays_the_document_aloud() {
        use craft_nora::Decision;
        let policy = policy();
        for id in ["review.readAloud", "readAloud.playPause", "readAloud.next", "readAloud.previous", "readAloud.stop", "readAloud.speed"] {
            assert_eq!(policy.check(id), Decision::Deny, "{id}");
        }
    }

    /// One command of Nora's, as the conversation hands it to the pane.
    fn play(app: &mut WordApp, command: &str, params: Value) -> Result<Value, String> {
        run(app, DocActionKind::Execute { command: command.into(), params })
    }

    /// Nora's command is refused, and says so.
    fn refused(app: &mut WordApp, command: &str, params: Value) -> String {
        let error = play(app, command, params).map(|v| v.to_string()).unwrap_err();
        assert!(error.contains("refused"), "{command}: {error}");
        error
    }

    /// Nora can't take back the person's work: her `edit.undo` and `edit.undoTo`, alone or in a batch, are refused
    /// before anything runs (the pane's Undo would then take back the person's own edit as Nora's).
    #[test]
    fn nora_never_undoes_the_person_s_work() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        a.run("text.insert", json!({"text": "mine"})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Tidy up");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        answer(&inbox, &take(&outbox)[0], json!({"thread_id": "t1"}));
        h.run_steps(6);
        let polls = take(&outbox);
        let actions = polls.iter().find(|r| r.path.ends_with("get_browser_actions")).expect("the actions are read");
        let asked = json!({"actions": [
            {"action": "doc_batch", "commands": [{"command": "text.insert", "params": {"text": " and Nora's"}}, {"command": "edit.undo"}], "callback_id": "c1"},
            {"action": "doc_execute", "command": "edit.undo", "callback_id": "c2"},
            {"action": "doc_execute", "command": "edit.undoTo", "params": {"count": 1}, "callback_id": "c3"}
        ]});
        answer(&inbox, actions, asked);
        h.run_steps(4);
        let told = take(&outbox);
        for callback in ["c1", "c2", "c3"] {
            let observation =
                told.iter().find(|r| r.path.ends_with("receive_observation") && r.body["callback_id"] == callback).expect("Nora gets an answer");
            assert_eq!(observation.body["observation"]["ok"], false, "{callback}");
            assert!(observation.body["observation"]["error"].as_str().unwrap_or_default().contains("refused"), "{callback}");
        }
        assert_eq!(text_of(h.state_mut()), "mine", "nothing ran: the person's typing is still there");
    }

    /// Nora's Redo would bring back what the person took back: refused.
    #[test]
    fn nora_s_redo_never_brings_back_what_the_person_undid() {
        let mut a = app(Services::default());
        a.run("text.insert", json!({"text": "mine"})).unwrap();
        a.run("edit.undo", json!({})).unwrap();
        refused(&mut a, "edit.redo", json!({}));
        assert_eq!(text_of(&mut a), "", "the person's Undo stands");
        assert!(a.session.can_redo(), "and their Redo is still theirs");
    }

    /// What the person types after Nora's change is theirs: the pane's Undo leaves it, and Nora's change, alone.
    #[test]
    fn undo_leaves_the_person_s_later_typing_alone() {
        let mut a = app(Services::default());
        type_in(&mut a, "Nora's");
        a.run("text.insert", json!({"text": " mine"})).unwrap();
        let error = run(&mut a, DocActionKind::Undo).map(|v| v.to_string()).unwrap_err();
        assert!(error.contains("Undo"), "{error}");
        assert_eq!(text_of(&mut a), "Nora's mine", "nothing is undone");
    }

    /// The person undoes Nora's change, then types: the undo step of their typing then holds the very document Nora's
    /// change started from, so a count of steps can't tell it from hers. The document's revision does: refused.
    #[test]
    fn undo_never_takes_back_the_person_s_edit_after_their_own_undo() {
        let mut a = app(Services::default());
        type_in(&mut a, "Nora's");
        a.run("edit.undo", json!({})).unwrap();
        a.run("text.insert", json!({"text": "mine"})).unwrap();
        assert!(run(&mut a, DocActionKind::Undo).is_err());
        assert_eq!(text_of(&mut a), "mine", "the person's typing stays");
    }

    #[test]
    fn undo_takes_back_nora_s_changes_one_after_the_other_but_never_past_the_person() {
        let mut a = app(Services::default());
        type_in(&mut a, "one");
        type_in(&mut a, " two");
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "one");
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "", "two changes in a row, both taken back");
        // Between two changes of Nora's, the person undoes the first and types: the first stays theirs to undo.
        type_in(&mut a, "first");
        a.run("edit.undo", json!({})).unwrap();
        a.run("text.insert", json!({"text": "mine"})).unwrap();
        type_in(&mut a, " second");
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "mine", "the newest change is taken back");
        assert!(run(&mut a, DocActionKind::Undo).is_err(), "the older one is behind the person's edit");
        assert_eq!(text_of(&mut a), "mine");
    }

    #[test]
    fn a_failed_action_keeps_the_change_before_it_undoable() {
        let mut a = app(Services::default());
        type_in(&mut a, "Nora's");
        let commands = vec![
            CommandCall { command: "text.insert".into(), params: json!({"text": " more"}) },
            CommandCall { command: "no.such.command".into(), params: json!({}) },
        ];
        assert!(run(&mut a, DocActionKind::Batch { commands }).is_err());
        assert_eq!(text_of(&mut a), "Nora's");
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "");
    }

    /// An action of Nora's that fails leaves the person's undo history as it was, their Redo included: the step it took
    /// before failing was not left there to redo, in place of theirs.
    #[test]
    fn a_failing_action_leaves_the_person_s_redo_as_it_was() {
        let mut a = app(Services::default());
        a.run("text.insert", json!({"text": "mine"})).unwrap();
        a.run("edit.undo", json!({})).unwrap();
        let commands = vec![
            CommandCall { command: "text.insert".into(), params: json!({"text": "Nora's"}) },
            CommandCall { command: "no.such.command".into(), params: json!({}) },
        ];
        assert!(run(&mut a, DocActionKind::Batch { commands }).is_err());
        assert_eq!(text_of(&mut a), "");
        a.run("edit.redo", json!({})).unwrap();
        assert_eq!(text_of(&mut a), "mine", "the person's Redo brings back what they undid");
    }

    /// At the undo limit (each new step drops the oldest), a long batch of Nora's that fails still leaves the person's
    /// history exactly as it was, and her change is still counted step by step for the pane's Undo.
    #[test]
    fn the_undo_limit_changes_nothing_to_nora_s_steps() {
        let mut a = app(Services::default());
        // The text is the steps': no capital letter AutoCorrect would make.
        a.session.prefs.autocorrect.enabled = false;
        for _ in 0..505 {
            a.run("text.insert", json!({"text": "x"})).unwrap();
            a.session.close_typing();
        }
        assert_eq!(a.session.undo_depth(), 500, "the history is full");
        let step = |n: usize| {
            if n.is_multiple_of(2) {
                CommandCall { command: "text.insert".into(), params: json!({"text": "n"}) }
            } else {
                CommandCall { command: "text.newParagraph".into(), params: json!({}) }
            }
        };
        let mut commands: Vec<CommandCall> = (0..6).map(step).collect();
        commands.push(CommandCall { command: "no.such.command".into(), params: json!({}) });
        assert!(run(&mut a, DocActionKind::Batch { commands }).is_err());
        assert_eq!(text_of(&mut a), "x".repeat(505));
        a.run("edit.undo", json!({})).unwrap();
        assert_eq!(text_of(&mut a), "x".repeat(504), "the person's Undo takes back their own last step, no step of the failed batch");
        a.run("edit.redo", json!({})).unwrap();
        run(&mut a, DocActionKind::Batch { commands: (0..2).map(step).collect() }).unwrap();
        assert_eq!(text_of(&mut a), format!("{}n\n", "x".repeat(505)));
        assert_eq!(run(&mut a, DocActionKind::Undo).unwrap()["undone"], 2, "both of Nora's steps, though the history stays at 500");
        assert_eq!(text_of(&mut a), "x".repeat(505));
    }

    /// A batch that fails leaves no formatting of Nora's waiting at the caret for the person's next typing.
    #[test]
    fn a_failing_batch_leaves_no_formatting_waiting_at_the_caret() {
        let mut a = app(Services::default());
        a.run("text.insert", json!({"text": "plain "})).unwrap();
        let commands = vec![
            CommandCall { command: "format.bold".into(), params: json!({}) },
            CommandCall { command: "no.such.command".into(), params: json!({}) },
        ];
        assert!(run(&mut a, DocActionKind::Batch { commands }).is_err());
        assert!(a.session.pending.is_none(), "the person types as before: {:?}", a.session.pending);
    }

    /// The person's clipboard, their Clipboard pane and the formatting their format painter holds are theirs: Nora
    /// neither reads nor changes them. A paste of what she gives herself is hers.
    #[test]
    fn nora_leaves_the_person_s_clipboard_alone() {
        let mut a = app(Services::default());
        a.run("text.insert", json!({"text": "secret"})).unwrap();
        a.run("select.all", json!({})).unwrap();
        a.run("edit.copy", json!({})).unwrap();
        a.run("caret.docEnd", json!({})).unwrap();
        let clipboard = a.session.clipboard_text.clone();
        assert_eq!(clipboard, "secret");
        for (command, params) in [
            ("edit.copy", json!({})),
            ("edit.cut", json!({})),
            ("edit.paste", json!({})),
            ("edit.pasteText", json!({})),
            ("edit.pasteMerge", json!({})),
            ("edit.pasteSpecial", json!({"as": "text"})),
            ("edit.clipboardItems", json!({})),
            ("edit.pasteClipboardItem", json!({"index": 0})),
            ("edit.pasteAllClipboard", json!({})),
            ("edit.deleteClipboardItem", json!({"index": 0})),
            ("edit.clearClipboard", json!({})),
            ("edit.copyFormat", json!({})),
            ("edit.pasteFormat", json!({})),
            ("edit.formatPainter", json!({})),
            // What the engine does not read as the text to paste leaves it to paste the person's clipboard.
            ("edit.paste", json!({"text": 5})),
            ("edit.pasteText", json!({"html": "<b>given</b>"})),
            ("edit.pasteSpecial", json!({"as": "formatted", "text": "secret"})),
            ("edit.pasteSpecial", json!({"as": "text", "html": "<b>given</b>"})),
            ("edit.pasteSpecial", json!({"as": "HTML", "rtf": "{\\rtf1 given}"})),
            ("edit.paste", json!({"text": ""})),
            // Replace with `^c` is the person's clipboard; without `with`, the Replace with text they typed last.
            ("edit.replaceAll", json!({"text": "secret", "with": "^c"})),
            ("edit.replace", json!({"text": "secret", "with": "a ^c"})),
            ("edit.replaceAll", json!({"text": "secret"})),
            ("edit.replaceAll", json!({"with": "theirs"})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(text_of(&mut a), "secret", "nothing of the person's clipboard was pasted");
        assert_eq!(a.session.clipboard_text, clipboard, "their clipboard holds what they copied");
        assert_eq!(a.session.clip_history.len(), 1, "and so does their Clipboard pane");
        assert!(a.session.painter.is_none(), "no formatting waits for their next selection");
        play(&mut a, "edit.paste", json!({"text": " and Nora's"})).unwrap();
        play(&mut a, "edit.pasteSpecial", json!({"as": "text", "text": " own"})).unwrap();
        play(&mut a, "edit.replaceAll", json!({"text": "own", "with": "own words"})).unwrap();
        assert_eq!(text_of(&mut a), "secret and Nora's own words");
    }

    /// The person's view, panes and windows are theirs, on the View tab or elsewhere: refused, and the view stays as
    /// they left it.
    #[test]
    fn nora_leaves_the_person_s_view_and_windows_alone() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        let view = serde_json::to_value(&a.session.view).unwrap();
        for (command, params) in [
            ("view.darkMode", json!({"value": true})),
            ("view.zoom", json!({"value": 200})),
            ("view.readMode", json!({})),
            ("view.outline", json!({})),
            ("view.newWindow", json!({})),
            ("view.split", json!({})),
            ("view.navigationPane", json!({"value": true})),
            ("table.viewGridlines", json!({"value": false})),
            ("styles.pane", json!({})),
            ("styles.inspector", json!({"value": true})),
            ("styles.apply", json!({})),
            ("review.markup", json!({"value": "noMarkup"})),
            ("review.showMarkup", json!({"value": false})),
            ("review.proofing", json!({"value": false})),
            ("review.hideInk", json!({"value": true})),
            ("fields.toggleCode", json!({})),
            ("developer.designMode", json!({"value": true})),
            ("draw.replay", json!({"value": true})),
            ("edit.clipboardPane", json!({"value": true})),
            ("smartArt.textPane", json!({"value": true})),
            ("outline.showLevel2", json!({})),
            ("outline.showAll", json!({})),
            ("outline.close", json!({})),
            ("edit.advancedFind", json!({"text": "Hello", "highlight": true})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(serde_json::to_value(&a.session.view).unwrap(), view, "the view is as the person left it");
        assert!(!a.session.find.highlight, "and nothing is highlighted in it");
    }

    /// The person's searches are theirs: Nora searches for the text she names, never for what the person searched for
    /// last, and their Find (what they searched for, their Replace with text, their options) stays as they left it.
    #[test]
    fn the_person_s_searches_stay_theirs() {
        let mut a = app(Services::default());
        a.run("text.insert", json!({"text": "Hello world, hello"})).unwrap();
        // The person searched with a regular expression.
        a.run("edit.find", json!({"text": "w.rld", "regex": true})).unwrap();
        a.run("caret.docEnd", json!({})).unwrap();
        let find = serde_json::to_value(&a.session.find).unwrap();
        for (command, params) in [
            ("edit.findNext", json!({})),
            ("edit.findPrevious", json!({})),
            ("edit.find", json!({"text": ""})),
            ("edit.advancedFind", json!({"highlight": null})),
            ("edit.advancedFind", json!({"text": "Hello", "highlight": 0})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(a.session.selected_text(), "", "nothing the person searched for was selected for Nora");
        // Nora's search starts from Find's defaults: the person's options don't change what her text means.
        let found = play(&mut a, "edit.find", json!({"text": "hello", "matchCase": true})).unwrap();
        assert_eq!(found["count"], 1, "{found}");
        play(&mut a, "edit.replaceAll", json!({"text": "o.", "with": "X"})).unwrap();
        assert_eq!(text_of(&mut a), "Hello world, hello", "`o.` is two characters to Nora, not the person's pattern");
        assert_eq!(serde_json::to_value(&a.session.find).unwrap(), find, "the person's Find is as they left it");
    }

    /// The tools that take the person's next clicks (a pen, the eraser, the lasso, the table pen, the format painter)
    /// and the pens of their gallery stay theirs. The same commands with a place to work on are edits.
    #[test]
    fn the_tools_for_the_person_s_next_clicks_stay_theirs() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        let draw = serde_json::to_value(&a.session.view.draw).unwrap();
        for (command, params) in [
            ("draw.pen", json!({})),
            ("draw.pencil", json!({"color": "FF0000"})),
            ("draw.highlighter", json!({})),
            ("draw.select", json!({})),
            ("draw.eraser", json!({})),
            ("draw.lasso", json!({})),
            // The lasso's own reading: only `clear: true` or `points` leave the tool where it is.
            ("draw.lasso", json!({"clear": false})),
            ("draw.lasso", json!({"clear": "yes"})),
            ("draw.addPen", json!({"kind": "pen", "color": "00FF00", "width": 2})),
            ("draw.removePen", json!({"index": 0})),
            ("draw.inkToShape", json!({"value": true})),
            ("table.draw", json!({})),
            ("table.eraser", json!({})),
            ("edit.formatPainter", json!({"sticky": true})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(serde_json::to_value(&a.session.view.draw).unwrap(), draw, "no tool in the person's hand, their pens as they were");
        assert!(a.canvas.table_tool.is_none(), "nor the table pen");
        assert!(a.session.painter.is_none(), "nor the format painter");
        for (command, params) in [
            ("draw.eraser", json!({"page": 0, "x": 10, "y": 10})),
            ("draw.lasso", json!({"clear": true})),
            ("table.draw", json!({"page": 0, "x0": 72, "y0": 300, "x1": 300, "y1": 400})),
        ] {
            let result = play(&mut a, command, params);
            assert!(!result.as_ref().is_err_and(|e| e.contains("refused")), "{command}: {result:?}");
        }
    }

    /// A macro runs its steps, and Repeat the last command, out of the pane's sight: refused.
    #[test]
    fn no_macro_nor_repeat_runs_a_command_out_of_the_pane_s_sight() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        let steps = json!([{"command": "view.darkMode", "params": {"value": true}}]);
        for (command, params) in [
            ("tools.macros", json!({"define": {"name": "Dark", "steps": steps}, "run": "Dark"})),
            ("tools.recordMacro", json!({"name": "Mine"})),
            ("edit.repeat", json!({})),
        ] {
            refused(&mut a, command, params);
        }
        assert!(!a.session.view.dark_mode, "no step of a macro ran");
        assert!(a.session.macros.is_empty() && a.session.recording.is_none(), "and no macro was made");
    }

    /// The person's settings (typing, AutoCorrect, keys, ribbon, how revisions and words are shown and counted), their
    /// dictionary and the fonts they added are theirs.
    #[test]
    fn the_person_s_settings_dictionary_and_fonts_stay_theirs() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        let prefs = a.session.prefs.clone();
        let keys = serde_json::to_value(&a.session.keymap).unwrap();
        let ribbon = serde_json::to_value(&a.session.ribbon).unwrap();
        for (command, params) in [
            ("text.overtype", json!({"value": true})),
            ("tools.autocorrect", json!({"add": {"from": "teh", "to": "Nora"}})),
            ("tools.customizeKeyboard", json!({"assign": {"command": "format.bold", "key": "Mod+Shift+K"}})),
            ("tools.customizeRibbon", json!({"hide": "Home"})),
            ("review.trackingOptions", json!({"balloons": "inline"})),
            ("review.wordCount", json!({"includeTextBoxes": true})),
            ("review.language", json!({"detect": false})),
            ("review.language", json!({"lang": "de-DE", "default": true})),
            ("review.addToDictionary", json!({"word": "qqnorazzword"})),
            ("review.ignoreAll", json!({"word": "qqnorazzword"})),
            ("font.add", json!({"data": "AAAA", "name": "Nora.ttf"})),
            ("font.remove", json!({"family": "Nora Sans"})),
        ] {
            refused(&mut a, command, params);
        }
        assert!(a.session.prefs == prefs, "the person's settings are as they were");
        assert_eq!(serde_json::to_value(&a.session.keymap).unwrap(), keys);
        assert_eq!(serde_json::to_value(&a.session.ribbon).unwrap(), ribbon);
        play(&mut a, "review.wordCount", json!({})).unwrap();
        // The language of the text itself is the document's: Nora sets it, the person's saved language stays.
        for params in [json!({"lang": "fr-CH"}), json!({"lang": "fr-CH", "default": false})] {
            play(&mut a, "review.language", params).unwrap();
        }
        assert!(a.session.prefs.default_language == prefs.default_language, "the saved language is the person's");
    }

    /// A file of the computer is reached by other names than `path`: the original of a comparison, the template the
    /// document is attached to. Refused.
    #[test]
    fn no_file_of_the_computer_is_reached_by_another_name() {
        let mut a = app(Services::default());
        type_in(&mut a, "Mine");
        a.session.doc.settings.attached_template = Some("/nonexistent/Nora.dotx".into());
        for (command, params) in [
            ("review.compare", json!({"text": "Theirs", "original": "/nonexistent/original.docx"})),
            ("review.combine", json!({"text": "Theirs", "original": "/nonexistent/original.docx"})),
            ("tools.updateStyles", json!({})),
            ("tools.updateStyles", json!({"data": 5})),
            ("tools.attachTemplate", json!({"path": "/nonexistent/Nora.dotx", "update": true})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(text_of(&mut a), "Mine");
    }

    /// A password of Nora's would lock the person out of their own document, or open it with one they gave nobody.
    #[test]
    fn no_password_of_nora_s_locks_the_person_out() {
        let mut a = app(Services::default());
        type_in(&mut a, "Mine");
        for params in
            [json!({"mode": "readOnly", "password": "secret"}), json!({"stop": true, "password": "guess"}), json!({"options": {"newPassword": "x"}})]
        {
            let error = refused(&mut a, "review.restrict", params);
            assert!(error.contains("password"), "{error}");
        }
        assert!(a.session.doc.settings.protection.is_none(), "nothing was protected");
        // Without a password the person lifts the protection as easily: allowed.
        play(&mut a, "review.restrict", json!({"mode": "comments"})).unwrap();
        assert_eq!(a.session.doc.settings.protection.as_deref(), Some("comments"));
    }

    /// Nora edits the document that is open: the mail merge commands that would replace it with a new one are refused.
    #[test]
    fn nora_never_replaces_the_document() {
        let mut a = app(Services::default());
        type_in(&mut a, "Mine");
        a.session.path = Some("/nonexistent/Mine.docx".into());
        let document = a.session.document_id();
        for (command, params) in [
            ("mailings.labels", json!({"text": "Nora"})),
            ("mailings.envelopes", json!({"delivery": "Nora\nSomewhere"})),
            // A comparison shown in a new document turns the one open into an untitled one, its file forgotten.
            ("review.compare", json!({"text": "Theirs", "showIn": "new"})),
            ("review.combine", json!({"text": "Theirs", "showIn": "revised"})),
        ] {
            refused(&mut a, command, params);
        }
        assert_eq!(a.session.document_id(), document, "the person's document is still the one open");
        assert_eq!(a.session.path.as_deref(), Some(std::path::Path::new("/nonexistent/Mine.docx")), "with its file");
        assert_eq!(text_of(&mut a), "Mine");
        // Compared in the document itself, the differences are tracked changes in it: an edit.
        play(&mut a, "review.compare", json!({"text": "Mine and theirs"})).unwrap();
        assert_eq!(a.session.path.as_deref(), Some(std::path::Path::new("/nonexistent/Mine.docx")));
        let added = play(&mut a, "mailings.envelopes", json!({"delivery": "Nora\nSomewhere", "add": true}));
        assert!(!added.as_ref().is_err_and(|e| e.contains("refused")), "an envelope added to the document is an edit: {added:?}");
    }

    /// A command that would ask the person something (a dialog, a file picker, the search box of a pane) needs the
    /// person: refused, and nothing opens in front of them.
    #[test]
    fn a_command_that_would_ask_the_person_something_is_refused() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        for (command, params) in [
            ("format.fontDialog", json!({})),
            ("para.dialog", json!({})),
            ("layout.pageSetup", json!({})),
            ("edit.findReplace", json!({})),
            ("edit.find", json!({})),
            ("edit.goto", json!({})),
            ("insert.picture", json!({})),
            ("insert.chart", json!({})),
            ("tools.search", json!({})),
        ] {
            refused(&mut a, command, params);
            assert!(a.dialog.is_none(), "{command}: no dialog in front of the person");
        }
        assert!(!a.session.view.nav_pane, "nor a pane opened for them");
        assert!(!a.ui.backstage);
        // With what it needs, the same command asks nothing, and runs.
        play(&mut a, "edit.find", json!({"text": "Hello"})).unwrap();
    }

    /// `dir` is a direction (Text Direction, moving in an equation), not a folder: no command of WordCraft reads a
    /// folder by `dir`.
    #[test]
    fn a_dir_parameter_is_a_direction_not_a_folder() {
        let mut a = app(Services::default());
        a.run("insert.textBox", json!({"text": "Label"})).unwrap();
        play(&mut a, "shape.textDirection", json!({"dir": "vert"})).unwrap();
    }

    /// An action that changed nothing (a command that only reads: no undo step, the document as it was) says so with
    /// `"changed": false`, so that the conversation offers no Undo for it; a change says nothing of the kind, and only
    /// the changes are kept for the pane's Undo.
    #[test]
    fn an_action_that_changed_nothing_says_so() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello");
        let read = play(&mut a, "document.text", json!({})).unwrap();
        assert_eq!(read["changed"], false, "{read}");
        assert_eq!(read["text"], "Hello", "beside what the command answers");
        let commands = vec![
            CommandCall { command: "document.selection".into(), params: json!({}) },
            CommandCall { command: "review.wordCount".into(), params: json!({}) },
        ];
        let batch = run(&mut a, DocActionKind::Batch { commands }).unwrap();
        assert_eq!(batch["changed"], false, "{batch}");
        assert_eq!(batch["result"].as_array().map(Vec::len), Some(2), "a list goes under `result`: {batch}");
        let typed = run(&mut a, DocActionKind::TypeText { text: " world".into(), paragraphs: false }).unwrap();
        assert!(typed.get("changed").is_none(), "a change says nothing of the kind: {typed}");
        let bold = play(&mut a, "format.bold", json!({})).unwrap();
        assert!(bold.get("changed").is_none(), "{bold}");
        // The pane's Undo takes back the changes, one after the other, never a reading.
        run(&mut a, DocActionKind::Undo).unwrap();
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "Hello");
        run(&mut a, DocActionKind::Undo).unwrap();
        assert_eq!(text_of(&mut a), "");
    }

    /// `path` also names a place in the document (a position's paragraph, a paragraph's): a list of numbers, which
    /// reaches no file.
    #[test]
    fn a_place_in_the_document_is_no_file() {
        let mut a = app(Services::default());
        type_in(&mut a, "Hello world");
        play(&mut a, "caret.set", json!({"pos": wordcraft_doc::Pos::body(0, 5)})).unwrap();
        assert_eq!(a.session.sel.focus.off, 5);
        play(&mut a, "select.range", json!({"anchor": wordcraft_doc::Pos::body(0, 6), "focus": wordcraft_doc::Pos::body(0, 11)})).unwrap();
        assert_eq!(a.session.selected_text(), "world");
        play(&mut a, "document.paragraph", json!({"path": [0]})).unwrap();
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

    /// The commands refused by their name alone, whatever their parameters: every entry of the pane's lists is a
    /// command of WordCraft (one renamed upstream would refuse nothing), and no other command of the engine is
    /// refused for its name. The lists are spelled out again here, so that a change to them changes this test too.
    #[test]
    fn the_commands_refused_by_name_are_the_person_s_and_no_others() {
        let registry = wordcraft_engine::cmd::registry();
        let ids: Vec<&str> = registry.all().iter().map(|spec| spec.id).collect();
        // A command an upstream merge adds is Nora's to run until someone decides otherwise: this count stops the merge
        // until the new commands have been looked at against the lists above (then it is updated).
        // v0.10.0 (638): the Spike and Load System Installed Fonts are the person's; the other thirteen are Nora's or refused
        // by their family (`file.`, `view.`).
        assert_eq!(ids.len(), 638, "WordCraft's commands changed: decide which new ones are the person's, then update this count");
        for entry in THE_PERSON_S {
            let known = match entry.strip_suffix('*') {
                Some(prefix) => ids.iter().any(|id| id.starts_with(prefix)),
                None => ids.contains(entry),
            };
            assert!(known, "{entry} is no command of WordCraft");
        }
        for family in REFUSED_FAMILIES.iter().filter(|family| **family != "ui.") {
            assert!(ids.iter().any(|id| id.starts_with(family)), "{family} is no family of WordCraft's commands");
        }
        let families = ["file.", "app.", "ui.", "view.", "edit.undo", "edit.redo", "review.readAloud", "readAloud.", "outline.showLevel"];
        let theirs = [
            "table.viewGridlines",
            "styles.pane",
            "styles.inspector",
            "smartArt.textPane",
            "edit.clipboardPane",
            "review.markup",
            "review.showMarkup",
            "review.trackingOptions",
            "review.proofing",
            "review.hideInk",
            "fields.toggleCode",
            "developer.designMode",
            "draw.replay",
            "outline.expand",
            "outline.collapse",
            "outline.toggle",
            "outline.showAll",
            "outline.firstLineOnly",
            "outline.showFormatting",
            "outline.close",
            "edit.repeat",
            "edit.copy",
            "edit.cut",
            "edit.clipboardItems",
            "edit.pasteClipboardItem",
            "edit.pasteAllClipboard",
            "edit.deleteClipboardItem",
            "edit.clearClipboard",
            "edit.copyFormat",
            "edit.pasteFormat",
            "edit.formatPainter",
            "edit.spike",
            "edit.insertSpike",
            "draw.pen",
            "draw.pencil",
            "draw.highlighter",
            "draw.select",
            "draw.addPen",
            "draw.removePen",
            "draw.inkToShape",
            "tools.macros",
            "tools.recordMacro",
            "text.overtype",
            "tools.autocorrect",
            "tools.customizeKeyboard",
            "tools.customizeRibbon",
            "review.addToDictionary",
            "review.ignoreAll",
            "font.add",
            "font.remove",
            "font.loadSystem",
        ];
        for id in &ids {
            let expected = families.iter().any(|family| id.starts_with(family)) || theirs.contains(id);
            assert_eq!(is_the_person_s(id), expected, "{id}");
        }
        // The editing commands next to them stay Nora's.
        for id in ["styles.inspectorClear", "edit.paste", "draw.stroke", "draw.erase", "draw.convertInk", "outline.promote", "review.newComment"] {
            assert!(ids.contains(&id) && !is_the_person_s(id), "{id}");
        }
    }

    /// The person reads why the pane's Undo is refused in their language, and the pane forgets Nora's older changes
    /// with it, as the conversation does: they lie under the person's edit too.
    #[test]
    fn a_refused_undo_says_why_in_the_person_s_language_and_forgets_nora_s_changes() {
        let mut a = app(Services::default());
        a.run("ui.language", json!({"value": "fr"})).unwrap();
        // As every frame does before the pane plays what Nora asks.
        crate::i18n::set_current(crate::i18n::Lang::from_pref(&a.ui.language));
        type_in(&mut a, "un");
        type_in(&mut a, " deux");
        a.run("text.insert", json!({"text": " trois"})).unwrap();
        let error = run(&mut a, DocActionKind::Undo).unwrap_err();
        assert_eq!(error, "Vous avez modifié le document après Nora : utilisez plutôt la fonction Annuler de l'éditeur.");
        assert!(a.nora.changes.is_empty(), "the older change of Nora's is forgotten too");
        assert_eq!(run(&mut a, DocActionKind::Undo).unwrap_err(), "Il n'y a aucune modification de Nora à annuler.");
        assert_eq!(text_of(&mut a), "un deux trois", "nothing was undone");
    }

    /// Only the requests of Nora's commands are kept from the interface: the person's own command still opens its
    /// dialog, after one of Nora's was refused for asking.
    #[test]
    fn the_person_s_own_dialogs_still_open() {
        let mut a = app(Services::default());
        refused(&mut a, "format.fontDialog", json!({}));
        assert!(a.nora.asked.is_none(), "the pane keeps the requests only while it plays a command");
        a.run("format.fontDialog", json!({})).unwrap();
        assert!(a.dialog.is_some(), "the person's Font dialog opens");
    }

    /// While Nora works, the pane's Send button is a Stop button: a click ends her turn there and then, the pane says
    /// so, and the next message can go.
    #[test]
    fn stop_ends_nora_s_turn() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Write a long report");
        h.run_steps(2);
        h.get_by_label("Send").click();
        h.run_steps(4);
        answer(&inbox, &take(&outbox)[0], json!({"thread_id": "t1"}));
        h.run_steps(4);
        assert!(h.state().nora.session.is_busy(), "Nora is at work");
        h.get_by_label("Stop").click();
        h.run_steps(4);
        assert!(!h.state().nora.session.is_busy(), "her turn ended");
        assert!(h.query_by_label("Stopped.").is_some(), "the pane says Nora was stopped");
        assert!(h.query_by_label("Send").is_some(), "the next message can go");
    }

    /// A call that got no answer at all is said in the interface language, not with the browser's English reason.
    #[test]
    fn no_answer_from_nora_is_said_in_the_interface_language() {
        let (services, outbox, inbox) = connected();
        let mut a = app(services);
        a.run("ui.language", json!({"value": "fr"})).unwrap();
        a.run("ui.nora", json!({"value": true})).unwrap();
        let mut h = harness(a);
        h.state_mut().nora.panel.set_draft("Bonjour");
        h.run_steps(2);
        h.get_by_label("Envoyer").click();
        h.run_steps(4);
        let sent = take(&outbox);
        let body = json!({"exception": "No answer from Nora: TypeError: Failed to fetch"}).to_string();
        inbox.lock().unwrap_or_else(|e| e.into_inner()).push(HttpResponse { id: sent[0].id, status: 0, body });
        h.run_steps(4);
        assert!(h.query_by_label("Nora n'a pas pu être jointe. Vérifiez la connexion et réessayez.").is_some());
        assert!(h.query_by_label("No answer from Nora: TypeError: Failed to fetch").is_none());
    }

    /// The page that hosts the editor names the file the document is: Nora is told its id with each message about it,
    /// only while the document is that file, and only an id that reads as one.
    #[test]
    fn the_context_carries_the_file_the_page_named() {
        let file: crate::NoraFile = Arc::default();
        let mut a = app(Services { nora_file: Some(file.clone()), ..Services::default() });
        assert_eq!(context_of(&mut a).document.id, None, "nothing named, nothing told");
        let name = context_of(&mut a).document.name;
        *file.lock().unwrap() = Some((name.clone(), Some("3fcaa7893c".into())));
        assert_eq!(context_of(&mut a).document.id.as_deref(), Some("3fcaa7893c"));
        let value: Value = serde_json::from_str(&context_of(&mut a).to_json_string()).unwrap();
        assert_eq!(value["document"]["id"], "3fcaa7893c", "{value}");
        *file.lock().unwrap() = Some(("Autre.docx".into(), Some("3fcaa7893c".into())));
        assert_eq!(context_of(&mut a).document.id, None, "the page's file is another document");
        *file.lock().unwrap() = Some((name.clone(), Some("../x".into())));
        assert_eq!(context_of(&mut a).document.id, None, "what is no id");
        *file.lock().unwrap() = Some((name, None));
        assert_eq!(context_of(&mut a).document.id, None, "a file the page names not");
        let mut b = app(Services::default());
        assert_eq!(context_of(&mut b).document.id, None, "a build with no page names no file");
    }

    /// A change of Nora's that leaves a link to a file, or a field that reads one, is taken back whole, and her answer
    /// says why; a link to a web page and the document's own fields stay.
    #[test]
    fn a_change_that_leads_outside_the_document_is_taken_back_and_said() {
        let mut a = app(Services::default());
        type_in(&mut a, "Tarifs ");
        let kept = text_of(&mut a);
        let link = |url: &str| DocActionKind::Execute { command: "insert.link".into(), params: json!({"url": url, "text": "catalogue"}) };
        let error = run(&mut a, link("C:\\Tarifs\\catalogue.pdf")).unwrap_err();
        assert!(error.starts_with("`insert.link` was undone"), "{error}");
        assert!(error.contains("a link to C:\\Tarifs\\catalogue.pdf (the document's text)"), "{error}");
        assert_eq!(text_of(&mut a), kept, "nothing of it is left");
        for url in ["\\\\srv\\share\\x.docx", "file:///C:/x.pdf", "#[Contrat.docx]Annexe"] {
            assert!(run(&mut a, link(url)).unwrap_err().contains("was undone"), "{url}");
        }
        let field = |instr: &str| DocActionKind::Execute { command: "insert.field".into(), params: json!({"instr": instr, "result": "x"}) };
        let error = run(&mut a, field("INCLUDETEXT \"C:\\\\x.docx\"")).unwrap_err();
        assert!(error.contains("a INCLUDETEXT field, which reaches outside the document"), "{error}");
        assert!(run(&mut a, field("HYPERLINK \"\\\\\\\\srv\\\\x\"")).unwrap_err().contains("a HYPERLINK field to"));
        assert_eq!(text_of(&mut a), kept);
        run(&mut a, link("https://example.com")).unwrap();
        run(&mut a, link("#Annexe")).unwrap();
        run(&mut a, field("PAGE")).unwrap();
        assert!(text_of(&mut a).contains("catalogue"), "a web page, a bookmark and a page number stay");
    }

    /// The person's own link to a file stays theirs: Nora making a word of it bold splits its run in two, and that is
    /// still the one link, not a new one.
    #[test]
    fn formatting_a_word_of_the_person_s_link_to_a_file_is_not_refused() {
        let mut a = app(Services::default());
        a.execute("insert.link", json!({"url": "\\\\srv\\cat.pdf", "text": "le catalogue"})).unwrap();
        run(&mut a, DocActionKind::SelectText { text: "catalogue".into(), occurrence: 1 }).unwrap();
        run(&mut a, DocActionKind::Execute { command: "format.bold".into(), params: json!({}) }).unwrap();
        assert_eq!(text_of(&mut a), "le catalogue");
    }
}
