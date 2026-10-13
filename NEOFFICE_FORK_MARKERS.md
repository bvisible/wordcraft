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
  available and offers no text box.

## Files that cannot carry a comment

| File | Why it is ours |
|---|---|
| `Cargo.lock` | Lists `craft-nora` and `craft-nora-ui` (git, from Neoffice's `craft-libs` fork) for the Nora pane. On a merge conflict, take upstream's file and run `cargo metadata`: cargo writes our two entries back. |
