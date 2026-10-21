# Neoffice fork markers

Map of **our** divergence from upstream (`storytold/wordcraft`) in this fork (`bvisible/wordcraft`, branch
`version-15`). Everything that can carry a comment carries a `//// Neoffice — …` marker in place:
`// //// Neoffice — …` in Rust (a bare `////` is an error for clippy), `# //// Neoffice — …` in TOML
and YAML, `<!-- //// Neoffice — … -->` in Markdown. This file holds the base of the divergence and
what cannot carry a comment.

At the next upstream merge, `grep -rn "////"` in the source plus this file give the complete
picture of what is ours and why.

- **Upstream**: `storytold/wordcraft`, branch `main`.
- **Base of the divergence**: `7dfa6b0ee2aeeafec004c51fdd2ab9a697da3cc4`, the tag `v0.6.1`, where `version-15` was cut on 2026-10-13.
  Move it at each upstream merge. The check works it out itself (the common ancestor with
  upstream's `main`), so this line is for the reader.

## What the fork adds

Three workflow files and one feature, all marked in place. The workflow files are new files that carry
their own `//// Neoffice — added file` header.

- `.github/workflows/neoffice-web.yml`: builds the browser version on a GitHub runner, started by
  hand, with the packaging script upstream's own release job runs. Upstream's workflows are
  disabled on this fork, because they need upstream's signing secrets.
- `.github/workflows/neoffice-markers.yml`: fails a push to `version-15` when a change to
  upstream's code since the common ancestor carries no marker.
- `.github/workflows/neoffice-ci.yml`: runs upstream's own quality gate, `cargo xtask ci`, on a
  GitHub runner (by hand, or on a push that touches code), because the maintainer's Mac has too
  little disk to run it for every change.
- **The Nora assistant pane** (Neoffice's assistant, in a pane at the right of the page), off until
  File › Options › Advanced › Assistant or the `ui.nora` command turns it on. It is built on
  `craft-nora` (the conversation with Nora, no I/O) and `craft-nora-ui` (the egui panel), two crates
  of Neoffice's `craft-libs` fork that depend on egui only. New file
  `crates/ui-egui/src/nora_pane.rs`; marked changes in `Cargo.toml` and `crates/ui-egui/Cargo.toml`
  (the two dependencies), `crates/ui-egui/src/lib.rs` (the `nora_pane` preference, the `ui.nora`
  command, `Services::nora_http` and `Services::nora_inbox` for the host's connection, the pane's
  state), `crates/ui-egui/src/panes.rs` (the pane) and `crates/ui-egui/src/options.rs` (the
  checkbox). Without a host connection, which is every build today, the pane says that Nora isn't
  available and offers no text box. With one, what Nora asks for in the document is played through
  `WordApp::execute`, editing commands only (`refusal` in `nora_pane.rs`): not those that open, save,
  close or replace a document (the mail merge's included, and a comparison shown in a new document),
  nor those that are the person's (the
  interface, the View tab and the view settings and panes of other tabs, their undo history and
  Repeat, their clipboard (Replace with `^c` included) and the format painter, their searches (a search
  names its text; the person's Find is put back after each command), Read Aloud, the tools for their next clicks and their
  pens, macros, their settings, dictionary and fonts), nor one with a parameter that names a file (as
  text: `path` is also a place in the document) or holds a password, at any depth. A command that asks
  the interface for a dialog, a file picker or a pane's search box is refused: while the pane plays a
  command, `WordApp::after_command` hands such requests to the pane instead of acting on them (a
  marked line in `crates/ui-egui/src/lib.rs`). Nora's commands also run with automation roots that
  grant no file at all (`AutomationRoots::none`, added in `crates/engine/src/file_access.rs` with its
  test). An action that fails puts back the document, its undo history and the person's view as they
  were. The pane's Undo takes back Nora's last change only while the document is still at the revision
  that change left (`Session::rev`), so it never takes back the person's own work, also after they
  undid Nora's change and typed; an action that changed nothing answers with `"changed": false` and
  leaves nothing for the pane's Undo (craft-nora offers no Undo for it from `9d28340` on; the `8fee6ab`
  pinned today still does). At the undo limit (500 steps) the pane counts Nora's steps with
  `Session::undo_pushed` and rolls a failed action back with `Session::edit_snapshot_whole`, both added
  in `crates/engine/src/session.rs` (marked): `undo_depth` stops growing there, and `edit_snapshot`
  keeps only the oldest four steps.
  Her strings are translated outside upstream's catalogs, in the new file
  `crates/ui-egui/src/i18n/neoffice.rs` (French), which `i18n::tr` falls back to (one marked line in
  `crates/ui-egui/src/i18n/mod.rs`).
  The browser build connects the pane with a `fetch` made with the hosting page's own session
  (new file `apps/wordcraft-web/src/web/nora.rs`; marked changes in `apps/wordcraft-web/src/web.rs`
  and the web-sys features of `apps/wordcraft-web/Cargo.toml`, and a re-export in
  `crates/ui-egui/src/lib.rs`): a page that asks for it with `?nora` gets the pane open and connected
  to the page's own origin; every other page behaves exactly as before. `?nora=<origin>` is taken only
  when it is the page's own origin, or a loopback origin from a loopback page (the workshop's stand-in
  server); any other origin the address names is refused with a console warning, and only the hosting
  page can name another one (`window.wordcraftNora.origin`). The CSRF token and the headers of
  `window.wordcraftNora` go only with calls to the page's own origin. The rules are in the new file
  `apps/wordcraft-web/src/nora_origin.rs`, tested natively; before, a link to an editor with
  `?nora=https://elsewhere.example` sent the person's messages, what Nora read of the document and the
  page's CSRF token there.
- **The URL flags of the browser build read by name** (new file `apps/wordcraft-web/src/flags.rs`,
  `URLSearchParams`; marked lines in `apps/wordcraft-web/src/web.rs` and `apps/wordcraft-web/src/main.rs`,
  which builds the two new modules for the native tests too): upstream found `webgl` and `sample`
  anywhere in the query, so `?author=webgl-fan` forced WebGL2 and `?nora=https://sample.example` opened
  the sample document.
- **The host bridge of the browser build** (new file `apps/wordcraft-web/src/web/host.rs`; marked
  changes in `apps/wordcraft-web/src/web.rs`, the web-sys features of `apps/wordcraft-web/Cargo.toml`
  and one line in `crates/ui-egui/src/i18n/neoffice.rs`): the `postMessage` contract of a page that
  frames the editor (`?host=parent`, same origin), the one upstream's GridCraft and PdfCraft already
  use, so one host page (the Drive of the Suite, NeoDesk's window) drives every Craft editor:
  `wordcraft:ready`, `wordcraft:open` (a file to edit), `wordcraft:save` (every file the editor would
  download), `wordcraft:dirty` and `wordcraft:failed`, plus `wordcraft:saved` (`ok: false` marks the
  document unsaved again and says why, `ok: true` marks it saved again when it has not changed since it was handed over: ours, upstream's bridges have no answer to a save) and
  `?author=` (the name that signs comments). Without `?host=parent` the editor behaves as before.
  Drop it when upstream ships its own bridge for WordCraft, keeping `wordcraft:saved`.

- **What a hosting page changes about the editor** (`Services::keep_text_format` and
  `Services::hide_community_links`, both set by `apps/wordcraft-web/src/web/host.rs`, so only when a page frames the
  editor with `?host=parent`): a document opened from a text file (`.txt`, `.md`) is saved under its own name and in its
  own format, and counts as saved (upstream turns it into a `.docx` and leaves it unsaved); the links to upstream's
  community (Discord) are not shown (title bar, Help tab, File menu, About dialog, Options), and the `ui.discord`
  command does nothing. Marked changes in `crates/ui-egui/src/lib.rs` (the two fields, `default_save_name`, the download
  branch of `execute`, `ui.discord`, three small functions and two tests), `chrome.rs`, `ribbon.rs`, `backstage.rs`,
  `dialogs.rs` and `options.rs`. Every other build behaves as before.

- **A hosting page opens the documents** (`Services::host_owns_documents`, set by
  `apps/wordcraft-web/src/web/host.rs`, so only when a page frames the editor with `?host=parent`): the editor does not
  switch to another document by itself. New, Open, Close, New from Template, Recover and the Mailings commands that
  make a new document are refused with a status message (`run` in `crates/ui-egui/src/lib.rs`, `switches_document`),
  File › Home, New and Open are not pages of the File menu (`backstage.rs`) and no Start screen shows, and a document
  dropped on the editor is not taken (`opens_as_document`, asked by `apps/wordcraft-web/src/web.rs`); the file the page
  hands over still opens (`drain_inbox` lifts the guard for it). Before, File › New or Open in a hosted window replaced
  the document of the page with another one the page knew nothing about. Every other build behaves as before.

- **Our product's name in our builds** (new file `crates/ui-egui/src/brand.rs`): `brand::name()` is
  `NEOFFICE_PRODUCT_NAME` when the build sets it (only the build step of `neoffice-web.yml`: `NeoDoc`), upstream's name
  otherwise, so upstream's builds and every test run are unchanged. `i18n::tr` passes its result through
  `brand::rebrand`, which swaps the whole word `WordCraft` in every language without touching the catalogs (the licence
  line keeps it); the title bar, the window title, two error messages and the page that failed to start call
  `brand::name()` (marked lines in `lib.rs`, `chrome.rs`, `i18n/mod.rs` and `apps/wordcraft-web/src/web.rs`). The page
  title and the loading text are ours in `apps/wordcraft-web/index.html`. Stored or compared names (storage folders,
  `WordCraft User`, `wordcraft:*` messages, `ai.storyteller.*`) and the native app keep upstream's.

## Files that cannot carry a comment

| File | Why it is ours |
|---|---|
| `Cargo.lock` | Lists `craft-nora` and `craft-nora-ui` (git, from Neoffice's `craft-libs` fork) for the Nora pane. On a merge conflict, take upstream's file and run `cargo metadata`: cargo writes our two entries back. |
