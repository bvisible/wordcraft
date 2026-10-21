// //// Neoffice — added file (no upstream equivalent)
//! The product name the interface shows.
//!
//! Neoffice ships this editor under its own name. Our browser builds set `NEOFFICE_PRODUCT_NAME`
//! at compile time (the build step of `.github/workflows/neoffice-web.yml`); without it, as in
//! upstream's builds and every test run, [`name`] is upstream's name and nothing changes.
//!
//! Upstream has no single source for its name, and its English strings are the keys of the
//! translation catalogs. So the translation lookup ([`crate::i18n::tr`]) passes its result
//! through [`rebrand`], which swaps the whole word for ours in every language, and the few
//! literals outside the catalogs call [`name`]. Names that are stored or compared (storage
//! folders, `WordCraft User`, the `wordcraft:*` messages of a hosting page) never come here.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Upstream's name for this app.
pub const UPSTREAM: &str = "WordCraft";

/// At most this many rebranded strings are kept (a catalog has about twenty per language). Past
/// it, a new string shows upstream's name rather than grow memory.
const MAX_REBRANDED: usize = 1024;

/// The name the interface shows: `NEOFFICE_PRODUCT_NAME` when the build sets it (not blank),
/// upstream's otherwise.
pub fn name() -> &'static str {
    match option_env!("NEOFFICE_PRODUCT_NAME").map(str::trim) {
        Some(ours) if !ours.is_empty() => ours,
        _ => UPSTREAM,
    }
}

/// `text`, the display form of the English `source`, with upstream's name (the whole word) as
/// [`name`]. The licence line keeps it: it credits upstream's contributors.
pub fn rebrand<'a>(source: &str, text: &'a str) -> &'a str {
    let ours = name();
    if ours == UPSTREAM || !text.contains(UPSTREAM) || source.contains("Copyright") {
        return text;
    }
    rebranded(text, ours).unwrap_or(text)
}

/// The rebranded form of `text`, made once and kept for the life of the app (bounded by
/// [`MAX_REBRANDED`]), so a string drawn every frame costs a lookup, not an allocation.
fn rebranded(text: &str, ours: &str) -> Option<&'static str> {
    static DONE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut done = DONE.get_or_init(Default::default).lock().ok()?;
    if let Some(&kept) = done.get(text) {
        return Some(kept);
    }
    if done.len() >= MAX_REBRANDED {
        return None;
    }
    let made: &'static str = replace_word(text, UPSTREAM, ours).leak();
    done.insert(text.to_owned(), made);
    Some(made)
}

/// `text` with every whole-word `from` replaced by `to`. Only ASCII letters and digits join a
/// word, so `WordCraftの` (Japanese) is replaced and `WordCrafts` or `NotWordCraft` are not.
fn replace_word(text: &str, from: &str, to: &str) -> String {
    let joins = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    let mut out = String::with_capacity(text.len() + to.len());
    let mut rest = text;
    let mut last: Option<char> = None;
    while let Some((before, after)) = rest.split_once(from) {
        out.push_str(before);
        let left = before.chars().next_back().or(last);
        out.push_str(if joins(left) || joins(after.chars().next()) { from } else { to });
        last = from.chars().next_back();
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_the_variable_nothing_changes() {
        if option_env!("NEOFFICE_PRODUCT_NAME").is_none_or(|n| n.trim().is_empty()) {
            assert_eq!(name(), UPSTREAM);
            assert_eq!(rebrand("About WordCraft", "À propos de WordCraft"), "À propos de WordCraft");
        }
    }

    #[test]
    fn replaces_the_whole_word_only() {
        assert_eq!(replace_word("About WordCraft", UPSTREAM, "NeoDoc"), "About NeoDoc");
        assert_eq!(replace_word("WordCraft's own browser", UPSTREAM, "NeoDoc"), "NeoDoc's own browser");
        assert_eq!(replace_word("WordCraftについて", UPSTREAM, "NeoDoc"), "NeoDocについて");
        assert_eq!(replace_word("关于 WordCraft", UPSTREAM, "NeoDoc"), "关于 NeoDoc");
        assert_eq!(replace_word("WordCrafts NotWordCraft", UPSTREAM, "NeoDoc"), "WordCrafts NotWordCraft");
        assert_eq!(replace_word("WordCraft, WordCraft.", UPSTREAM, "NeoDoc"), "NeoDoc, NeoDoc.");
        assert_eq!(replace_word("WordCraftWordCraft", UPSTREAM, "NeoDoc"), "WordCraftWordCraft");
        assert_eq!(replace_word("getartcraft.com/apps/wordcraft", UPSTREAM, "NeoDoc"), "getartcraft.com/apps/wordcraft");
        assert_eq!(replace_word("", UPSTREAM, "NeoDoc"), "");
    }

    #[test]
    fn a_rebranded_string_is_made_once() {
        let first = rebranded("Browse in WordCraft… (test)", "NeoDoc");
        assert_eq!(first, Some("Browse in NeoDoc… (test)"));
        let again = rebranded("Browse in WordCraft… (test)", "NeoDoc");
        assert!(first.zip(again).is_some_and(|(a, b)| std::ptr::eq(a, b)), "the same string, not a new one");
    }
}
