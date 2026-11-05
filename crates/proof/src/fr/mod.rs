// //// Neoffice — added file (no upstream equivalent)
//! French proofing. The dictionary is the one installed (Hunspell `fr_FR`, `fr`…, read like every other language's) or,
//! when there is none — always on the web, which reads no folder — the Grammalecte dictionary built into the program
//! ([`builtin`]). It reads through the affix engine ([`crate::affix`]): `FLAG long`, `FULLSTRIP` and `BREAK` are what
//! it needs of it, so that `sont`, `est-ce` and `parle-t-il` are words. Suggestions use the dictionary's own tables and
//! add the French elision (`jai` → `j'ai`, [`suggest`]). Grammar is limited to generic repeated-word and spacing checks
//! with French messages ([`grammar`]); there are no French-specific grammar or typography rules, no hyphenation
//! patterns (the English fallback is used) and no sentence-capital rule.

mod builtin;
pub mod grammar;
mod suggest;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::affix::AffixDict;
use crate::slot::DictSlot;

pub use suggest::suggest;

/// File names of a French Hunspell dictionary: Debian's and LibreOffice's (`fr_FR`, `fr`), then the other regions'
/// (all of them are read for French of any region).
const NAMES: &[&str] = &["fr_FR", "fr-FR", "fr", "fr_CH", "fr_BE", "fr_CA", "fr_LU"];

/// The French dictionary: an installed one found on first use, else the built-in one (see [`DictSlot`]).
pub(crate) static DICT: DictSlot<AffixDict> = DictSlot::with_builtin(NAMES, "French", builtin::LABEL, builtin::load);

/// The French dictionary: the installed Hunspell one (read on first use), one given with [`install_dictionary`], or the
/// built-in one.
pub fn dictionary() -> Arc<AffixDict> {
    DICT.get()
}

/// Use this French dictionary from now on (Hunspell `.aff` and `.dic` text, decoded to UTF-8), for example one the user
/// picked. On error the current dictionary stays.
pub fn install_dictionary(aff: &str, dic: &str) -> Result<(), String> {
    DICT.install(aff, dic)
}

/// Is a French dictionary available (spelling is checked)? Always, unless the built-in one failed to load.
pub fn has_dictionary() -> bool {
    DICT.has_dictionary()
}

/// The elided words that take an apostrophe before a vowel (`l'été`, `qu'il`, `jusqu'à`): what stands before it in a word the
/// dictionary doesn't have whole but the user dictionary may (`d'Apple`).
const ELISIONS: &[&str] = &["c", "d", "j", "l", "m", "n", "s", "t", "qu", "jusqu", "lorsqu", "puisqu", "quoiqu", "presqu"];

/// Is `word` spelled correctly in French? Numbers, single letters, short words in capitals (acronyms: `TVA`, `AVS`), words
/// with digits (`1er`, `2e`, `B2B`), units with an exponent (`km²`) and words in other scripts are accepted, and so is every word when no French dictionary is
/// available. A hyphenated word is correct when the dictionary has it whole (`peut-être`, `c'est-à-dire`) or breaks it into
/// correct parts (`est-ce`, `parle-t-il`, `dis-le-moi`, `demi-cantons`); parts and elided words (`d'Apple`) may be words of
/// the user dictionary, and an acronym or a number after an elision (`l'OTAN`, `d'AT`) is left alone as it is alone.
pub fn is_correct(word: &str) -> bool {
    is_correct_with(&dictionary(), word)
}

/// [`is_correct`] with the File › Options › Proofing choices: with Ignore words in UPPERCASE off, short words in capitals are
/// checked against the dictionary too, and with Ignore words that contain numbers off, so are words with digits.
pub fn is_correct_with_options(word: &str, opts: &crate::Options) -> bool {
    check(&dictionary(), word, opts)
}

/// [`is_correct`] against a given dictionary (an empty one accepts everything).
pub fn is_correct_with(d: &AffixDict, word: &str) -> bool {
    check(d, word, &crate::Options::default())
}

/// [`is_correct_with_options`] against a given dictionary.
fn check(d: &AffixDict, word: &str, opts: &crate::Options) -> bool {
    let Some(w) = crate::spelling::word_to_check(d, word, opts) else {
        return true;
    };
    if part_known(d, w, opts) {
        return true;
    }
    // The dictionary breaks a hyphenated word itself (`BREAK`); the user dictionary is not the dictionary's, so its
    // words, and the elisions in front of them, are looked for part by part too (`Kubernetes-cluster`, `d'Apple-store`).
    w.contains('-') && w.split('-').all(|p| p.is_empty() || part_known(d, p, opts))
}

/// Is `w`, a word or one part of a hyphenated one, right?
fn part_known(d: &AffixDict, w: &str, opts: &crate::Options) -> bool {
    if known(d, w) {
        return true;
    }
    // A unit with an exponent (`km²`, `cm³`): the unit is the word, as in the text of a surface or a volume.
    if w.strip_suffix(['²', '³']).is_some_and(|unit| !unit.is_empty() && known(d, unit)) {
        return true;
    }
    // After an elision, what needs no lookup on its own (an acronym: `l'OTAN`, `l'HP-IB`, a number) or is the person's
    // (`d'Apple`).
    w.split_once(['\'', '’']).is_some_and(|(head, tail)| {
        ELISIONS.iter().any(|e| e.eq_ignore_ascii_case(head))
            && tail.chars().count() >= 2
            && (crate::in_user_dictionary(&tail.to_lowercase()) || crate::spelling::word_to_check(d, tail, opts).is_none())
    })
}

/// Is one word in the dictionary or the user dictionary?
fn known(d: &AffixDict, w: &str) -> bool {
    d.check(w) || crate::in_user_dictionary(&w.to_lowercase())
}
