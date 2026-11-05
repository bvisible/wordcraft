// //// Neoffice — added file (no upstream equivalent)
//! French suggestions: the shared ranking ([`crate::affix_suggest`]) with the dictionary's own MAP/REP/TRY/KEY tables and its
//! `OCONV` (typographic apostrophes), after what only French has: the elision a writer forgets (`jai` → `j'ai`, `lhomme` →
//! `l'homme`), the hyphen of a compound written in one piece (`peutêtre` → `peut-être`), and the wrong part of a hyphenated
//! word (`parle-t-ill` → `parle-t-il`).

use crate::affix::AffixDict;
use crate::affix_suggest::{COST_MISTYPE, Tuning};

/// The shared ranking with what French adds: a writer seldom gets the first letter of a word wrong, so a candidate that
/// changes it ranks after those that fix the rest (`rendez-vou` → `rendez-vous`, not `rendez-cou`).
const TUNING: Tuning = Tuning { mistype_cost, ..Tuning::DEFAULT };

/// A word of at most this many letters, once its apostrophe is missing, is read as the elision first.
const SHORT_WORD: usize = 5;

fn mistype_cost(_: &[char], at: usize, _: char) -> u32 {
    if at == 0 { COST_MISTYPE + 2 } else { COST_MISTYPE }
}

/// Spelling suggestions for a French word, best first, at most `max`, in the word's letter case.
pub fn suggest(word: &str, max: usize) -> Vec<String> {
    suggest_in(&super::dictionary(), word, max)
}

/// [`suggest`] against a given dictionary.
pub fn suggest_in(d: &AffixDict, word: &str, max: usize) -> Vec<String> {
    let word = word.trim();
    if word.is_empty() || max == 0 || d.is_empty() || word.chars().count() > crate::affix_suggest::MAX_LETTERS || word.contains(char::is_whitespace) {
        return Vec::new();
    }
    // The dictionary reads a typographic apostrophe as a straight one, and shows suggestions with the typographic one.
    let word = word.replace('’', "'");
    let mut out: Vec<String> = Vec::new();
    let add = |out: &mut Vec<String>, s: String| {
        let s = d.convert_output(&s).into_owned();
        if !out.contains(&s) {
            out.push(s);
        }
    };
    // What only French has: a missing hyphen comes first, and so does a missing apostrophe in a short word (`jai`, `cest`, `quil`),
    // where the elision is what was meant far likelier than not; in a longer word it comes after the best of the shared ranking
    // (`dosier` is `dossier` before `d'osier`), and the elision of a word that also needs a correction after the two best
    // (`dacord` → `d'accord`).
    let (mut later, mut guesses): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    if word.contains('-') {
        for s in hyphenated(d, &word, max) {
            add(&mut out, s);
        }
    } else {
        for s in hyphen_inserted(d, &word) {
            add(&mut out, s);
        }
        if !word.contains('\'') {
            let (sure, corrected) = elided(d, &word);
            if word.chars().count() <= SHORT_WORD {
                for s in sure {
                    add(&mut out, s);
                }
            } else {
                later = sure;
            }
            guesses = corrected;
        }
    }
    // The shared ranking shows its suggestions through `OCONV` already.
    for (i, s) in crate::affix_suggest::suggest_in(d, &word, max, &TUNING).into_iter().enumerate() {
        if i == 1 {
            for l in std::mem::take(&mut later) {
                add(&mut out, l);
            }
        }
        if i == 2 {
            for g in std::mem::take(&mut guesses) {
                add(&mut out, g);
            }
        }
        if !out.contains(&s) {
            out.push(s);
        }
    }
    for l in later.into_iter().chain(guesses) {
        add(&mut out, l);
    }
    out.retain(|s| *s != word && *s != word.replace('\'', "’"));
    out.truncate(max);
    out
}

/// The elisions: `j'ai` for `jai`, where the elided form is a dictionary word (sure), and the elisions of a corrected rest
/// (`d'accord` for `dacord`: `acord` is not a word, its first suggestion is).
fn elided(d: &AffixDict, word: &str) -> (Vec<String>, Vec<String>) {
    let mut heads: Vec<&str> = super::ELISIONS.to_vec();
    heads.sort_by_key(|h| std::cmp::Reverse(h.len()));
    let lower = word.to_lowercase();
    let (mut sure, mut corrected) = (Vec::new(), Vec::new());
    for head in heads {
        let n = head.chars().count();
        if lower.starts_with(head) && word.chars().count() > n {
            let split = word.char_indices().nth(n).map_or(word.len(), |(i, _)| i);
            if let (Some(h), Some(rest)) = (word.get(..split), word.get(split..)) {
                let candidate = format!("{h}'{rest}");
                if d.can_suggest(&candidate) {
                    sure.push(candidate);
                } else if rest.chars().count() >= 3 {
                    for s in crate::affix_suggest::suggest_in(d, rest, 2, &TUNING) {
                        let candidate = format!("{h}'{}", s.replace('’', "'"));
                        if !s.contains(' ') && d.can_suggest(&candidate) {
                            corrected.push(candidate);
                        }
                    }
                }
            }
        }
    }
    (sure, corrected)
}

/// The compounds written in one piece: `peut-être` for `peutêtre`, where the dictionary has the hyphenated word itself.
fn hyphen_inserted(d: &AffixDict, word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in word.char_indices().skip(1) {
        if let (Some(a), Some(b)) = (word.get(..i), word.get(i..)) {
            let candidate = format!("{a}-{b}");
            if d.contains_suggestible(&candidate) || d.contains_suggestible(&candidate.to_lowercase()) {
                out.push(candidate);
            }
        }
    }
    out
}

/// A hyphenated word with one wrong part: the part is corrected and the word put together again, if the dictionary takes it.
fn hyphenated(d: &AffixDict, word: &str, max: usize) -> Vec<String> {
    let parts: Vec<&str> = word.split('-').collect();
    let wrong: Vec<usize> = parts.iter().enumerate().filter(|(_, p)| !p.is_empty() && !d.check(p)).map(|(i, _)| i).collect();
    let [k] = wrong.as_slice() else { return Vec::new() };
    let Some(part) = parts.get(*k) else { return Vec::new() };
    let mut out = Vec::new();
    for s in crate::affix_suggest::suggest_in(d, part, max, &TUNING) {
        let s = s.replace('’', "'");
        let rebuilt: Vec<&str> = parts.iter().enumerate().map(|(i, p)| if i == *k { s.as_str() } else { p }).collect();
        let joined = rebuilt.join("-");
        if !s.contains(' ') && d.can_suggest(&joined) {
            out.push(joined);
        }
    }
    out
}
