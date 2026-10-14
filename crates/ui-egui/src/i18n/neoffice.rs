// //// Neoffice — added file (no upstream equivalent): Neoffice's own interface strings, translated here so that upstream's catalogs stay as they are.

//! The strings Neoffice added to WordCraft (the Nora pane and its switch in File › Options), kept
//! outside upstream's catalogs. [`super::tr`] asks here when the language's own catalog has no entry
//! for a string, so an upstream merge never touches these, and a catalog that gains the same English
//! string wins over this table.
//!
//! French is the first language here: Neoffice's customers read it. The other languages show these
//! strings in English until someone adds them.

use super::Lang;

/// Neoffice's strings in French (Swiss French, formal "vous"), English first.
pub const FRENCH: &[(&str, &str)] = &[
    ("Show the Nora assistant pane (experimental)", "Afficher le volet de l'assistant Nora (expérimental)"),
    ("Assistant", "Assistant"),
    ("Nora isn't available in this version yet", "Nora n'est pas encore disponible dans cette version"),
    ("New conversation", "Nouvelle conversation"),
    ("Undo the last change Nora made to the document", "Annuler la dernière modification faite par Nora dans le document"),
    ("Ask Nora…", "Demander à Nora…"),
    ("Nora is working…", "Nora travaille…"),
    (
        "Ask Nora to write, rewrite, translate or format your document.",
        "Demandez à Nora d'écrire, de réécrire, de traduire ou de mettre en forme votre document.",
    ),
    ("Nora wants to:", "Nora souhaite :"),
    ("Allow", "Autoriser"),
    ("Refuse", "Refuser"),
    ("Read the document", "Lire le document"),
    ("Run {command}", "Exécuter {command}"),
    ("Run {count} commands", "Exécuter {count} commandes"),
    ("Select \"{text}\"", "Sélectionner \"{text}\""),
    ("Type \"{text}\"", "Saisir \"{text}\""),
    ("Look at page {page}", "Consulter la page {page}"),
    ("Save the document", "Enregistrer le document"),
    ("Undo the last change", "Annuler la dernière modification"),
    ("Send", "Envoyer"),
    ("Couldn't save {name}: {error}", "Impossible d'enregistrer {name} : {error}"),
];

/// `s` in `lang`, when Neoffice has translated it.
pub fn plain(lang: Lang, s: &str) -> Option<&'static str> {
    match lang.code() {
        "fr" => FRENCH.iter().find(|(english, _)| *english == s).map(|(_, french)| *french),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::catalog::placeholders;

    #[test]
    fn every_french_string_keeps_the_placeholders_of_its_english() {
        for (english, french) in FRENCH {
            assert_eq!(placeholders(english), placeholders(french), "{english:?}");
            assert_eq!(french.trim(), *french, "stray whitespace in {french:?}");
            assert!(!french.contains("..."), "use … rather than three dots: {french:?}");
        }
    }

    #[test]
    fn the_english_strings_are_listed_once() {
        let mut seen = std::collections::HashSet::new();
        for (english, _) in FRENCH {
            assert!(seen.insert(*english), "{english:?} is listed twice");
        }
    }

    #[test]
    fn only_french_has_them_for_now() {
        let fr = crate::i18n::Lang::all().find(|l| l.code() == "fr");
        assert_eq!(fr.and_then(|l| plain(l, "New conversation")), Some("Nouvelle conversation"));
        assert_eq!(plain(crate::i18n::Lang::EN, "New conversation"), None);
        assert_eq!(fr.and_then(|l| plain(l, "A string upstream has")), None);
    }
}
