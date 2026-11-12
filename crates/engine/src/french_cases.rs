// //// Neoffice — added file (no upstream equivalent)
//! French spelling cases, run against the dictionary this build carries ([`crate::french`]) through `craft-proof`'s public API: real French
//! sentences, correct and wrong, the elisions, inversions, capitals, ligatures, units, Swiss words, the suggestions, the user's dictionary and
//! the paragraph in two languages. They are test data written for this repository (they were the cases of the fork's first French checker,
//! before `crates/proof` moved to craft-libs); the identifiers and the comments are English as everywhere else.

use wordcraft_proof::{IssueKind, Options, ProofLang, check_text, is_correct_in, is_correct_in_with, suggest_in};

/// Swiss French (every French region shares the dictionary), with the dictionary handed over first.
fn french() -> ProofLang {
    crate::french::install();
    ProofLang::from_tag(Some("fr-CH"))
}

/// Every word looked up, none skipped: words in capitals, with digits or short are checked against the dictionary like any other
/// (the defaults accept them without a look, which would make a test of them say nothing).
const STRICT: Options = Options { ignore_uppercase: false, ignore_numbers: false, ignore_internet: false };

fn is_right(word: &str) -> bool {
    is_correct_in_with(word, french(), &STRICT)
}

/// The words of `text` flagged as misspelled when it is checked as French.
fn flagged(text: &str) -> Vec<&str> {
    check_text(text, &[(0..text.len(), french())])
        .into_iter()
        .filter(|i| i.kind == IssueKind::Spelling)
        .filter_map(|i| text.get(i.start..i.end))
        .collect()
}

fn assert_all_correct(sentences: &[&str]) {
    for s in sentences {
        assert_eq!(flagged(s), Vec::<&str>::new(), "{s}");
    }
}

#[test]
fn administrative_and_legal_french_is_accepted() {
    assert_all_correct(&[
        "Les conditions générales de vente s'appliquent à toutes les commandes passées auprès de notre société.",
        "Veuillez trouver ci-joint la facture relative à votre commande du 3 mars, payable dans les trente jours.",
        "Le Conseil fédéral a adopté aujourd'hui le message concernant la révision de la loi sur l'énergie.",
        "Le preneur s'engage à restituer les locaux en bon état à l'échéance du contrat de bail.",
        "Les cantons sont compétents pour autant que la Constitution ne les ait pas attribués à la Confédération.",
        "Nous vous remercions de votre confiance et restons à votre disposition pour tout renseignement complémentaire.",
    ]);
}

#[test]
fn literary_french_with_elisions_inversions_and_ligatures_is_accepted() {
    assert_all_correct(&[
        "C'est-à-dire qu'il faudra peut-être attendre jusqu'à septante jours, voire nonante.",
        "Parle-t-il vraiment de nous ? Est-ce que tu viens demain ? Dis-le-moi, va-t-on au marché ?",
        "L'État n'a pas tardé à agir ; l'œuvre du cœur, elle, reste à faire.",
        "Qu’est-ce que c’est ? Aujourd’hui, l’été commence tôt, et l’hiver s’en va.",
        "Les demi-cantons, les années-là et les arrière-grands-parents n'avaient pas de téléphone portable.",
        "Elle fut heureuse, ils étaient contents, nous serons là et j'irai plus tard : ils ont dit qu'ils sont prêts.",
    ]);
}

/// Things that failed before the engine read `FLAG long`, `FULLSTRIP` and `BREAK`.
#[test]
fn verbs_that_replace_their_whole_stem_and_hyphenated_words_are_words() {
    for w in
        ["sont", "ont", "serai", "irai", "était", "fut", "est-ce", "parle-t-il", "va-t-on", "dis-le-moi", "demi-cantons", "c'est-à-dire", "peut-être"]
    {
        assert!(is_correct_in(w, french()), "{w}");
    }
    for w in ["est-cx", "parle-t-ill", "sontt", "irais-tt", "peutêtre"] {
        assert!(!is_correct_in(w, french()), "{w}");
    }
}

#[test]
fn capitals_ligatures_and_apostrophes() {
    for w in [
        "ÉTAT",
        "À",
        "ŒUVRE",
        "Œuvre",
        "œuvre",
        "cœur",
        "CŒUR",
        "Été",
        "L'état",
        "L’État",
        "D'ABORD",
        "L'ÉTÉ",
        "qu'il",
        "qu’il",
        "jusqu'à",
        "aujourd'hui",
        "Aujourd’hui",
        "l'homme",
        "l’homme",
        "d'un",
        "j'ai",
        "n'est",
        "s'il",
    ] {
        assert!(is_right(w), "{w}");
    }
    for w in ["oeuvre", "coeur", "lhomme", "jai", "l'xyzzy", "etat", "ete", "ETAT", "ECOLE", "OEUVRE", "L'ETE", "D'ABORDD"] {
        assert!(!is_right(w), "{w}");
    }
}

#[test]
fn feminine_plural_and_swiss_words() {
    for w in [
        "étudiante",
        "étudiantes",
        "chevaux",
        "heureuse",
        "belles",
        "nouveaux",
        "septante",
        "huitante",
        "nonante",
        "natel",
        "Genève",
        "Berne",
        "Suisse",
    ] {
        assert!(is_correct_in(w, french()), "{w}");
    }
}

#[test]
fn ordinals_units_and_numbers_follow_the_options() {
    // The dictionary has the ordinals and the units themselves: they pass with every word looked up. Acronyms and a letter-digit
    // code are left to the options that skip them, and nothing else.
    for w in ["1er", "2e", "3es", "1re", "20e", "XXe", "km", "kg", "cm", "mm", "TVA", "AVS"] {
        assert!(is_right(w) || matches!(w, "TVA" | "AVS" | "20e" | "XXe"), "{w}");
    }
    for w in ["B2B", "2026", "TVA", "AVS", "XQZRT", "zqx2"] {
        assert!(is_correct_in(w, french()), "{w}: skipped by the default options");
    }
    for w in ["XQZRT", "zqx2"] {
        assert!(!is_right(w), "{w}: looked up, not a word");
    }
}

/// The cases the French dictionary is asked about every day, one per line of a Swiss office: a latin phrase, a unit with an exponent,
/// a Roman numeral and its ordinal, a feminine, an inverted verb.
#[test]
fn everyday_cases() {
    for w in
        ["km²", "m²", "m³", "cm³", "XIXe", "XXe", "Ier", "a priori", "ad hoc", "vice versa", "statu quo", "in situ", "de facto", "quorum", "bonus"]
    {
        for part in w.split(' ') {
            assert!(is_correct_in(part, french()), "{w}: {part}");
        }
    }
    for w in ["étudiante", "belle", "heureuse", "directrice", "nouvelles", "chevaux", "yeux", "œil", "travaux", "bijoux", "vieille", "folle"] {
        assert!(is_correct_in(w, french()), "{w}");
    }
    for w in [
        "aimerions",
        "mangeaient",
        "finissions",
        "recevrait",
        "voudrais",
        "pourrions",
        "écrivîmes",
        "fussions",
        "eussiez",
        "buvons",
        "mourront",
        "résolu",
    ] {
        assert!(is_correct_in(w, french()), "{w}");
    }
    for w in ["km²x", "zqx²", "wxkm²"] {
        assert!(!is_correct_in(w, french()), "{w}");
    }
}

#[test]
fn real_mistakes_are_flagged() {
    assert_eq!(flagged("Le systeme informatique n'est pas disponnible depuis hier."), ["systeme", "disponnible"]);
    assert_eq!(flagged("Nous avons bien recu votre demende et nous vous répondrons aprés les fêtes."), ["recu", "demende", "aprés"]);
    assert_eq!(flagged("J'ai fait une erreure dans l'adress du client."), ["erreure", "l'adress"]);
}

#[test]
fn suggestions_put_the_right_word_among_the_first_three() {
    for (wrong, right) in [
        ("systeme", "système"),
        ("dosier", "dossier"),
        ("dacord", "d’accord"),
        ("cest", "c’est"),
        ("quil", "qu’il"),
        ("evenement", "événement"),
        ("aprés", "après"),
        ("maintenent", "maintenant"),
        ("developpement", "développement"),
        ("apeler", "appeler"),
        ("ecole", "école"),
        ("Etat", "État"),
        ("jai", "j’ai"),
        ("Jai", "J’ai"),
        ("lhomme", "l’homme"),
        ("peutêtre", "peut-être"),
        ("parle-t-ill", "parle-t-il"),
        ("recevoire", "recevoir"),
    ] {
        let found = suggest_in(wrong, 3, french());
        assert!(found.contains(&right.to_string()), "{wrong}: {found:?}");
    }
}

#[test]
fn suggestions_are_shown_with_typographic_apostrophes_and_never_the_word_itself() {
    let found = suggest_in("l'ete", 8, french());
    assert!(found.iter().all(|s| !s.contains('\'')), "{found:?}");
    assert!(found.contains(&"l’été".to_string()), "{found:?}");
    for word in ["systeme", "qu’ill", "zqxwv", "a-b-c", "x"] {
        assert!(!suggest_in(word, 8, french()).contains(&word.to_string()), "{word}");
    }
    for word in ["", " ", "deux mots", &"é".repeat(60)] {
        assert!(suggest_in(word, 8, french()).is_empty(), "{word:?}");
    }
    assert!(suggest_in("systeme", 0, french()).is_empty() && suggest_in("systeme", 2, french()).len() <= 2);
}

#[test]
fn the_user_dictionary_reaches_parts_and_elisions() {
    assert!(!is_correct_in("Zqfrancais", french()) && !is_correct_in("d'Zqfrancais", french()));
    wordcraft_proof::add_word("Zqfrancais");
    assert!(is_correct_in("zqfrancais", french()) && is_correct_in("Zqfrancais", french()));
    assert!(is_correct_in("d'Zqfrancais", french()) && is_correct_in("l’Zqfrancais", french()), "after an elision");
    assert!(is_correct_in("Zqfrancais-règle", french()) && is_correct_in("règle-Zqfrancais", french()), "as a part");
    assert!(is_correct_in("d'Zqfrancais-règle", french()) && is_correct_in("règle-d’Zqfrancais", french()), "after an elision, as a part");
    assert!(!is_correct_in("Zqfrancais-xqzzv", french()) && !is_correct_in("x'Zqfrancais", french()), "a wrong part, or not an elision");
    wordcraft_proof::remove_word("Zqfrancais");
    assert!(!is_correct_in("Zqfrancais", french()));
}

/// An acronym after an elision is an acronym (`l'OTAN`); a word that is not one still has to be right after it. Units take
/// their plural through a prefix's continuation classes (`watts`).
#[test]
fn acronyms_after_elisions_and_unit_plurals() {
    for w in ["l'OTAN", "l’UEFA", "d'IBM", "lorsqu'IBM", "L'ONU", "l'HP-IB", "watts", "kelvins", "milliwatts"] {
        assert!(is_correct_in(w, french()), "{w}");
    }
    let strict = Options { ignore_uppercase: false, ..Options::default() };
    // (The dictionary itself takes `l'chat`: its elision prefix has no condition on the next letter.)
    for w in ["d'Artagnan", "l'xyzzy", "l'OTANNNN", "x'OTAN", "wattss"] {
        assert!(!is_correct_in(w, french()), "{w}");
    }
    assert!(!is_correct_in_with("l'XQZRT", french(), &strict) && is_correct_in_with("l'XQZRT", french(), &Options::default()));
}

#[test]
fn every_french_region_shares_one_dictionary_and_one_message() {
    for tag in ["fr", "fr-FR", "fr-CH", "fr-BE", "fr-CA", "fr-LU", "fr-MC", "fra"] {
        assert!(wordcraft_proof::has_dictionary(Some(tag)), "{tag}");
        assert!(wordcraft_proof::is_correct_for("septante", Some(tag)) && !wordcraft_proof::is_correct_for("septnate", Some(tag)), "{tag}");
        assert_eq!(wordcraft_proof::missing_dictionary(Some(tag)), None, "{tag}: the dictionary is built in");
        assert_eq!(wordcraft_proof::unreadable_dictionary(Some(tag)), None, "{tag}");
    }
    let text = "Une fôte ici";
    let issues = check_text(text, &[(0..text.len(), french())]);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!((issues[0].message.as_str(), issues[0].lang), ("Faute d’orthographe possible", french()));
}

#[test]
fn a_paragraph_in_two_languages_is_proofed_in_each() {
    let text = "Voici une phrase correcte. This sentence has a mispelled word. Et une faute: baucoup.";
    let english = text.find("This").unwrap_or(0)..text.find(" Et une").unwrap_or(text.len());
    let issues = check_text(text, &[(0..english.start, french()), (english.clone(), ProofLang::En), (english.end..text.len(), french())]);
    let spelling: Vec<(&str, ProofLang)> =
        issues.iter().filter(|i| i.kind == IssueKind::Spelling).filter_map(|i| Some((text.get(i.start..i.end)?, i.lang))).collect();
    assert_eq!(spelling, [("mispelled", ProofLang::En), ("baucoup", french())]);
}

#[test]
fn repeated_words_are_reported_in_french() {
    let text = "Il a a mangé le le gâteau, nous nous régalons.";
    let issues = check_text(text, &[(0..text.len(), french())]);
    let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
    assert_eq!(messages, ["Mot répété : « a »", "Mot répété : « le »"], "{issues:?}");
}

/// The elision outranks a correction only where it is the likelier thought: `jai` is `j'ai`, `dosier` is `dossier`.
#[test]
fn the_elision_ranks_by_how_short_the_word_is() {
    let first = |w: &str| suggest_in(w, 3, french()).first().cloned();
    assert_eq!(first("jai").as_deref(), Some("j’ai"));
    assert_eq!(first("cest").as_deref(), Some("c’est"));
    assert_eq!(first("dosier").as_deref(), Some("dossier"));
    assert!(suggest_in("dosier", 4, french()).contains(&"d’osier".to_string()));
    // A word that has an apostrophe already is not elided again.
    assert!(suggest_in("l'hommee", 4, french()).iter().all(|s| s.matches('’').count() <= 1), "{:?}", suggest_in("l'hommee", 4, french()));
}
