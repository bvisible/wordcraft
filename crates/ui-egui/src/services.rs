use super::state::DefaultApp;
use super::{FileDialogAnswer, FileDialogRequest, async_saves, paste_picture, printing};

/// Platform services injected by the host (file dialogs, file I/O).
#[derive(Default)]
pub struct Services {
    /// Desktop: discover configured printers on a worker.
    pub printers: Option<printing::Discover>,
    /// Desktop: submit the PDF with the chosen settings on a worker.
    pub print_native: Option<printing::Submit>,
    /// Pick a file to open; the argument says what for (`document`, `picture`).
    pub pick_open: Option<Box<dyn Fn(&str) -> Option<String>>>,
    /// Pick a path to save to, given a suggested name.
    pub pick_save: Option<Box<dyn Fn(&str) -> Option<String>>>,
    /// Desktop: show a native file dialog without blocking the UI thread (#94); used instead of
    /// `pick_open` / `pick_save` when set. The host sends the picked path (`Ok(None)` when
    /// cancelled) through the returned channel once the user answers, and wakes the UI; a dropped
    /// sender counts as cancelled. An `Err` says no system dialog could be shown, and the app shows
    /// its built-in file browser instead (#576). The app shows one dialog at a time ([`file_dialogs`]).
    pub file_dialog: Option<Box<dyn Fn(FileDialogRequest) -> std::sync::mpsc::Receiver<FileDialogAnswer>>>,
    /// Web: open a file picker; the file arrives later through `inbox`.
    pub open_async: Option<Box<dyn Fn(&str)>>,
    /// Web: files (name, bytes) delivered asynchronously (picker, drag and drop).
    pub inbox: Option<Inbox>,
    /// Web: pictures pasted from the browser's clipboard (the bytes; the name is ignored). Kept
    /// apart from `inbox` because a paste only lands in the document when the page is the edit
    /// target (#1349), where a dropped or opened file always lands.
    pub pasted: Option<Inbox>,
    /// Web: hand bytes to the browser as a download. An error means no download started (the
    /// browser can't tell the page whether the user then kept the file).
    pub download: Option<Box<dyn Fn(&str, &[u8]) -> Result<(), String>>>,
    /// Web: saves that `download` started and that finish later (the browser's save picker,
    /// #1315). The hook starts a ticket for such a save and reports how it ended; until then the
    /// document's save isn't confirmed, and a New/Open held back by the Save Changes prompt waits.
    pub async_saves: Option<async_saves::AsyncSaves>,
    /// Web, framed by a page that keeps the files (`?host=parent`): set alongside `download`,
    /// Save, Save As and Export hand the bytes to that page instead, with whether it was Save As
    /// ([`HostSaveFn`]: true for a `file.saveAs` in a format that keeps everything, which File ›
    /// Export to such a format runs too, and for a Save under a name the page never sent; false
    /// for Save and for copies: the PDF and PNG exports and Save As in a format that doesn't keep
    /// everything). Save then writes the opened file
    /// under its own name and format, as on the desktop, and a Save or Save As that keeps
    /// everything makes that name the document's file, so the next Save goes to it.
    pub host_save: Option<HostSaveFn>,
    /// Web: told whether the document has unsaved changes after each pass, for the browser's
    /// leave-page guard (`beforeunload` runs between frames and can't ask the app).
    pub on_dirty: Option<Box<dyn Fn(bool)>>,
    /// Web: hand PDF bytes to the browser's print dialog (no download or intermediate file).
    /// Desktop uses `print_native` instead. An error means nothing was opened.
    pub print: Option<Box<dyn Fn(&[u8]) -> Result<(), String>>>,
    /// Desktop: the picture on the system clipboard, if any — egui's paste only carries text
    /// (#45). Read when Paste finds no text; see [`paste_picture`].
    pub clipboard_picture: Option<Box<dyn Fn() -> Option<paste_picture::ClipboardPicture>>>,
    /// Whether the system has animation effects on (Windows: Animation effects; macOS: Reduce
    /// motion; Linux and the BSDs: the desktop's setting; browsers: `prefers-reduced-motion`), or
    /// `None` when it doesn't say. What "Use system setting" follows ([`theme::Animations`]).
    /// Asked every frame, so a change in the system's settings shows at once: the host answers
    /// from memory and does any slow asking elsewhere. Without the hook, animation effects stay on.
    pub animations_enabled: Option<Box<dyn Fn() -> Option<bool>>>,
    /// Desktop: the text on the system clipboard, if any. Ctrl+V gets it from egui's paste event;
    /// the ribbon and context-menu Paste read it here (#584).
    pub clipboard_text: Option<Box<dyn Fn() -> Option<String>>>,
    /// Desktop: the HTML on the system clipboard — another program's formatted copy (a Word
    /// document, a web page) — if any. Paste keeps its formatting (#687); see [`system_clipboard`].
    pub clipboard_html: Option<Box<dyn Fn() -> Option<String>>>,
    /// Desktop: put HTML on the system clipboard with its plain-text alternative (html, text, and
    /// for a copy of one equation alone its MathML, else empty), so other programs keep the
    /// formatting (#1523) and equations (#816). The MathML goes in the platform's MathML formats
    /// as well, which Word reads a pasted equation from (#1297). `true` when the HTML and text are
    /// there; otherwise the copy's text goes through egui as usual.
    pub set_clipboard_html: Option<Box<dyn Fn(&str, &str, &str) -> bool>>,
    /// Desktop: the RTF on the system clipboard, where the platform offers it (#687).
    pub clipboard_rtf: Option<Box<dyn Fn() -> Option<String>>>,
    /// Desktop: the names of the formats on the system clipboard (MIME types, Windows format
    /// names), where the platform lists them, so a paste that finds nothing it can use says what
    /// is there instead of "empty" (#833).
    pub clipboard_formats: Option<Box<dyn Fn() -> Vec<String>>>,
    /// Web: keep a font the user added (stored file name, original bytes) for the next visit
    /// (#582). Desktop fonts are stored by the font database itself, in the app's font folder.
    pub store_font: Option<Box<dyn Fn(&str, &[u8])>>,
    /// Web: forget a stored font (its stored file name) once the user removes it.
    pub forget_font: Option<Box<dyn Fn(&str)>>,
    /// Web: request browser to query and load local installed fonts (`window.queryLocalFonts()`).
    pub load_system_fonts: Option<Box<dyn Fn()>>,
    /// Desktop: File › Options › General › Make WordCraft the default for Word documents (#295),
    /// only from that button (never a programmatic command). Windows opens the Settings app's
    /// Default apps page (apps can't set defaults themselves); Linux and BSD set it with
    /// `xdg-mime`. `None` (web, macOS) hides the button; macOS shows how to do it in Finder
    /// instead. An error means nothing changed.
    pub make_default_app: Option<Box<dyn Fn() -> Result<DefaultApp, String>>>,
    // //// Neoffice — the connection of the Nora assistant pane. Nora (the agent) lives on the Neoffice
    // server, so the pane needs the host's session and credentials to reach her: the host makes the HTTP
    // call and delivers the answer through `nora_inbox`. Without one (every build today) the pane says
    // that Nora isn't available and offers no text box.
    /// Make one call for the Nora pane: a `POST` of a JSON body to a path of the Neoffice instance. The
    /// host adds the origin and the credentials of its session.
    pub nora_http: Option<Box<dyn Fn(craft_nora::HttpRequest)>>,
    /// The answers to [`Services::nora_http`], delivered asynchronously by the host.
    pub nora_inbox: Option<NoraInbox>,
    // //// Neoffice — the file a hosting page named with the document it opened (the Drive's file id), which the Nora
    // pane tells Nora with each message about that document. Without one (every other build) the pane names no file.
    /// The hosting page's last file: its name, and its id for it when it named one.
    pub nora_file: Option<NoraFile>,
    // //// Neoffice — the host opens a page of the person's instance for the pane (a card's Open, a link of a reply):
    // the path only (`/drive/f/…`), already checked by craft-nora (`instance_path`); the host knows the instance. Without
    // it the pane shows a link as its words.
    /// Open a page of the person's instance, by its path, the host's own way.
    pub nora_open: Option<Box<dyn Fn(&str)>>,
    // //// Neoffice — the host carries a dictation to the server (`nora.api.stt.transcribe`, with the person's credentials):
    // the pane then offers its microphone, where the browser can record. Off for every other build.
    pub nora_dictation: bool,
    // //// Neoffice — what a hosting page (the Drive of the Suite, Neodesk's window) changes about the editor: a text
    // file the person opened is saved as text rather than turned into a Word document, the links to the upstream
    // project's community (Discord) are not shown, and the page, not the editor, opens documents. All off, as
    // upstream has them, for every other build.
    /// A document opened from a text file (`.txt`, `.md`) is saved under its own name and in its own format.
    pub keep_text_format: bool,
    /// Hide the links to the upstream project's community (Discord).
    pub hide_community_links: bool,
    // //// Neoffice — see above: the page opens the documents.
    /// The hosting page opens, creates and closes documents, and is the only window: File › Home, New and Open are
    /// not offered, and a command that would open, replace, close or switch to another document (or open another
    /// window) is refused (the page's own file still opens). It also stores what the editor saves ([`crate::host_saves`]).
    pub host_owns_documents: bool,
    // //// Neoffice — the files the hosting page hands over, apart from `inbox` (what the person picks or drops): only
    // these open with the guard of `host_owns_documents` lifted.
    /// The files a hosting page sends to be the document, delivered asynchronously like [`Services::inbox`].
    pub host_inbox: Option<Inbox>,
    /// Desktop: the app's own user templates folder (`Templates` in the settings folder, #2852),
    /// used while File › Options names none. File › New lists personal templates from it and Save
    /// as Word Template starts in it. `None` (web) hides personal templates.
    pub templates_dir: Option<std::path::PathBuf>,
}

/// Files delivered asynchronously.
pub type Inbox = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

// //// Neoffice — answers to the Nora pane's calls, delivered asynchronously by the host like `Inbox`.
pub type NoraInbox = std::sync::Arc<std::sync::Mutex<Vec<craft_nora::HttpResponse>>>;
// //// Neoffice — the file the hosting page sent last, and its id for it (`Services::nora_file`).
pub type NoraFile = std::sync::Arc<std::sync::Mutex<Option<(String, Option<String>)>>>;

/// [`Services::host_save`]: `(file name, bytes, save_as)`. An error means the page didn't get
/// the file, and the save failed.
pub type HostSaveFn = Box<dyn Fn(&str, &[u8], bool) -> Result<(), String>>;
