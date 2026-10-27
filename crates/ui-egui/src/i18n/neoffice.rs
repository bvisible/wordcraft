// //// Neoffice — added file (no upstream equivalent): Neoffice's own interface strings, translated here so that upstream's catalogs stay as they are.

//! The strings Neoffice added to WordCraft (the Nora pane and its switch in File › Options), kept
//! outside upstream's catalogs. [`super::tr`] asks here when the language's own catalog has no entry
//! for a string, so an upstream merge never touches these, and a catalog that gains the same English
//! string wins over this table.
//!
//! French is the first language here: Neoffice's customers read it. The other languages show these
//! strings in English until someone adds them, except the two the interface looks up with the Nora pane
//! closed (see [`COMPLETE_CATALOGS`]): upstream keeps nine catalogs complete and its audit fails on a string
//! they lack.

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
    ("{name} can't be inserted as a picture: nothing was inserted.", "{name} ne peut pas être inséré comme image : rien n'a été inséré."),
    ("{name} can't be added as a font: nothing was added.", "{name} ne peut pas être ajouté comme police : rien n'a été ajouté."),
    (
        "{name} can't be loaded as a recipient list: nothing was loaded.",
        "{name} ne peut pas être chargé comme liste de destinataires : rien n'a été chargé.",
    ),
    ("This document is opened from the page that hosts the editor.", "Ce document est ouvert depuis la page qui héberge l'éditeur."),
    ("There is nothing of Nora's to undo.", "Il n'y a aucune modification de Nora à annuler."),
    (
        "You changed the document after Nora did: use the editor's Undo instead.",
        "Vous avez modifié le document après Nora : utilisez plutôt la fonction Annuler de l'éditeur.",
    ),
    ("Stopped.", "Arrêté."),
    ("Nora did not answer in time.", "Nora n'a pas répondu à temps."),
    ("Nora could not be reached. Check the connection and try again.", "Nora n'a pas pu être jointe. Vérifiez la connexion et réessayez."),
    ("Your session has expired. Sign in again.", "Votre session a expiré. Reconnectez-vous."),
    (
        "The server could not answer (error {status}). Try again in a moment.",
        "Le serveur n'a pas pu répondre (erreur {status}). Réessayez dans un instant.",
    ),
    ("Nora's answer could not be read.", "La réponse de Nora n'a pas pu être lue."),
    (RESUME.0, "Cet onglet était en arrière-plan : Nora n'a pas pu agir dans le document."),
    (RESUME.1, "Reprendre"),
    (CORRECTION.0, "Rien n'a été fait dans le document : Nora n'a pas pu y agir à temps. Redemandez si vous en avez encore besoin."),
    (CORRECTION.1, "Tout n'a pas été fait : une partie n'a pas pu s'exécuter dans le document à temps."),
    (BUTTON.0, "Demander à Nora"),
    (BUTTON.1, "Nora, l’assistante Neoffice"),
];

/// The strings the interface looks up whatever the Nora pane does (its ribbon group and its switch in
/// File › Options), in the languages whose catalogs upstream keeps complete: its audit
/// (`i18n::audit`) fails on a string they have no entry for. English first, as in [`FRENCH`].
const COMPLETE_CATALOGS: &[(&str, &[(&str, &str)])] = &[
    ("ru", &[("Assistant", "Ассистент"), ("Show the Nora assistant pane (experimental)", "Показывать область ассистента Nora (экспериментально)")]),
    ("hu", &[("Assistant", "Asszisztens"), ("Show the Nora assistant pane (experimental)", "Nora-asszisztens panel megjelenítése (kísérleti)")]),
    ("tr", &[("Assistant", "Asistan"), ("Show the Nora assistant pane (experimental)", "Nora asistan bölmesini göster (deneysel)")]),
    ("bg", &[("Assistant", "Асистент"), ("Show the Nora assistant pane (experimental)", "Показване на панела на асистента Nora (експериментално)")]),
    // v0.10.0's audit keeps five more catalogs complete.
    ("it", &[("Assistant", "Assistente"), ("Show the Nora assistant pane (experimental)", "Mostra il riquadro dell'assistente Nora (sperimentale)")]),
    ("sv", &[("Assistant", "Assistent"), ("Show the Nora assistant pane (experimental)", "Visa Nora-assistentpanelen (experimentell)")]),
    ("hr", &[("Assistant", "Asistent"), ("Show the Nora assistant pane (experimental)", "Prikaži okno asistenta Nora (eksperimentalno)")]),
    ("pl", &[("Assistant", "Asystent"), ("Show the Nora assistant pane (experimental)", "Pokaż panel asystenta Nora (eksperymentalny)")]),
    ("sl", &[("Assistant", "Pomočnik"), ("Show the Nora assistant pane (experimental)", "Prikaži podokno pomočnika Nora (eksperimentalno)")]),
];

/// What the pane says under a request lost to a tab in the background (craft-libs `ResumeWords`), and its button.
const RESUME: (&str, &str) = ("This tab was in the background: Nora could not act in the document.", "Resume");

/// [`RESUME`] in every other language of the interface (translated by Haiku, 24.10.2026): the code, the sentence, the
/// button. French is in [`FRENCH`].
const RESUME_WORDS: &[(&str, &str, &str)] = &[
    ("de", "Dieser Tab war im Hintergrund: Nora konnte im Dokument nichts tun.", "Fortsetzen"),
    ("it", "Questa scheda era in background: Nora non ha potuto agire nel documento.", "Riprendi"),
    ("es", "Esta pestaña estaba en segundo plano: Nora no pudo actuar en el documento.", "Reanudar"),
    ("pt-br", "Esta aba estava em segundo plano: a Nora não pôde agir no documento.", "Retomar"),
    ("nl", "Dit tabblad stond op de achtergrond: Nora kon niet in het document handelen.", "Hervatten"),
    ("da", "Denne fane var i baggrunden: Nora kunne ikke handle i dokumentet.", "Genoptag"),
    ("sv", "Den här fliken var i bakgrunden: Nora kunde inte agera i dokumentet.", "Återuppta"),
    ("pl", "Ta karta była w tle: Nora nie mogła działać w dokumencie.", "Wznów"),
    ("cs", "Tato karta byla na pozadí: Nora nemohla v dokumentu jednat.", "Pokračovat"),
    ("sl", "Ta zavihek je bil v ozadju: Nora ni mogla delovati v dokumentu.", "Nadaljuj"),
    ("hr", "Ova kartica bila je u pozadini: Nora nije mogla djelovati u dokumentu.", "Nastavi"),
    ("sr", "Ова картица била је у позадини: Нора није могла да делује у документу.", "Настави"),
    ("sr-latn", "Ova kartica bila je u pozadini: Nora nije mogla da deluje u dokumentu.", "Nastavi"),
    ("bg", "Този раздел беше във фонов режим: Nora не можа да действа в документа.", "Продължи"),
    ("ru", "Эта вкладка была в фоновом режиме: Nora не смогла выполнить действие в документе.", "Продолжить"),
    ("uk", "Ця вкладка була у фоновому режимі: Nora не змогла виконати дію в документі.", "Продовжити"),
    ("et", "See vahekaart oli taustal: Nora ei saanud dokumendis tegutseda.", "Jätka"),
    ("hu", "Ez a lap a háttérben volt: Nora nem tudott műveletet végezni a dokumentumban.", "Folytatás"),
    ("ro", "Această filă era în fundal: Nora nu a putut acționa în document.", "Reia"),
    ("tr", "Bu sekme arka plandaydı: Nora belgede işlem yapamadı.", "Devam et"),
    ("vi", "Thẻ này đang ở chế độ nền: Nora không thể thao tác trong tài liệu.", "Tiếp tục"),
    ("ja", "このタブはバックグラウンドにあったため、Nora はドキュメントを操作できませんでした。", "再開"),
    ("ko", "이 탭이 백그라운드에 있어서 Nora가 문서에서 작업할 수 없었습니다.", "재개"),
    ("zh-hans", "此标签页处于后台：Nora 无法在文档中执行操作。", "恢复"),
    ("zh-hant", "此分頁位於背景：Nora 無法在文件中執行操作。", "恢復"),
];

/// What the pane says of a reply whose turn could not act in the document in time (craft-libs `CorrectionWords`): nothing
/// was done, part was not done.
const CORRECTION: (&str, &str) = (
    "Nothing was done in the document: Nora could not act in it in time. Ask again if you still need it.",
    "Not all of this was done: part of it could not run in the document in time.",
);

/// [`CORRECTION`] in every other language of the interface (translated by Haiku, 24.10.2026): the code, then the two
/// sentences. French is in [`FRENCH`].
const CORRECTION_WORDS: &[(&str, &str, &str)] = &[
    (
        "de",
        "Im Dokument wurde nichts geändert: Nora konnte darin nicht rechtzeitig etwas tun. Fragen Sie erneut, falls Sie es noch brauchen.",
        "Nicht alles wurde ausgeführt: Ein Teil konnte rechtzeitig nicht im Dokument ausgeführt werden.",
    ),
    (
        "it",
        "Nel documento non è stato fatto nulla: Nora non ha potuto agire in tempo. Richieda di nuovo se ne ha ancora bisogno.",
        "Non tutto è stato fatto: una parte non ha potuto essere eseguita nel documento in tempo.",
    ),
    (
        "es",
        "No se hizo nada en el documento: Nora no pudo actuar en él a tiempo. Vuelva a pedirlo si todavía lo necesita.",
        "No se hizo todo: una parte no pudo ejecutarse en el documento a tiempo.",
    ),
    (
        "pt-br",
        "Nada foi feito no documento: Nora não conseguiu agir nele a tempo. Peça novamente se ainda precisar.",
        "Nem tudo foi feito: parte não pôde ser executada no documento a tempo.",
    ),
    (
        "nl",
        "Er is niets in het document gedaan: Nora kon er niet op tijd in handelen. Vraag het opnieuw als u het nog nodig hebt.",
        "Niet alles is gedaan: een deel kon niet op tijd in het document worden uitgevoerd.",
    ),
    (
        "da",
        "Der blev ikke gjort noget i dokumentet: Nora kunne ikke nå at handle i det. Spørg igen, hvis du stadig har brug for det.",
        "Ikke alt blev gjort: en del af det kunne ikke køres i dokumentet i tide.",
    ),
    (
        "sv",
        "Ingenting gjordes i dokumentet: Nora kunde inte agera i det i tid. Fråga igen om du fortfarande behöver det.",
        "Inte allt gjordes: en del kunde inte köras i dokumentet i tid.",
    ),
    (
        "pl",
        "Nic nie zostało zrobione w dokumencie: Nora nie mogła w nim zadziałać na czas. Ponów prośbę, jeśli nadal jest potrzebna.",
        "Nie wszystko zostało zrobione: część nie mogła zostać wykonana w dokumencie na czas.",
    ),
    (
        "cs",
        "V dokumentu nebylo nic provedeno: Nora v něm nemohla včas zasáhnout. Pokud to ještě potřebujete, zadejte žádost znovu.",
        "Ne všechno bylo provedeno: část se v dokumentu nepodařilo spustit včas.",
    ),
    (
        "sl",
        "V dokumentu nič ni bilo narejeno: Nora ni mogla pravočasno ukrepati. Če še potrebujete, zahtevo ponovite.",
        "Vse ni bilo narejeno: del ni mogel biti pravočasno izveden v dokumentu.",
    ),
    (
        "hr",
        "U dokumentu nije ništa učinjeno: Nora u njemu nije mogla pravodobno djelovati. Ako i dalje trebate, zatražite ponovno.",
        "Nije sve učinjeno: dio nije mogao biti pravodobno izvršen u dokumentu.",
    ),
    (
        "sr",
        "У документу ништа није урађено: Нора у њему није могла да делује на време. Поново затражите ако и даље треба.",
        "Није све урађено: део није могао да се изврши у документу на време.",
    ),
    (
        "sr-latn",
        "U dokumentu ništa nije urađeno: Nora u njemu nije mogla da deluje na vreme. Ponovo zatražite ako i dalje treba.",
        "Nije sve urađeno: deo nije mogao da se izvrši u dokumentu na vreme.",
    ),
    (
        "bg",
        "В документа нищо не беше направено: Нора не можа да действа в него навреме. Заявете отново, ако още ви е нужно.",
        "Не всичко беше направено: част от него не можа да се изпълни в документа навреме.",
    ),
    (
        "ru",
        "В документе ничего не сделано: Нора не успела внести изменения вовремя. Повторите запрос, если он всё ещё нужен.",
        "Сделано не всё: часть не удалось выполнить в документе вовремя.",
    ),
    (
        "uk",
        "У документі нічого не зроблено: Нора не встигла внести зміни вчасно. Повторіть запит, якщо він досі потрібен.",
        "Зроблено не все: частину не вдалося виконати в документі вчасно.",
    ),
    (
        "et",
        "Dokumendis ei tehtud midagi: Nora ei saanud seal õigel ajal tegutseda. Kui seda veel vajate, küsige uuesti.",
        "Kõike ei tehtud: osa ei saanud dokumendis õigeks ajaks käivituda.",
    ),
    (
        "hu",
        "A dokumentumban semmi sem történt: Nora nem tudott időben beavatkozni. Ha még szükség van rá, kérje újra.",
        "Nem minden történt meg: egy részét nem tudták időben lefuttatni a dokumentumban.",
    ),
    (
        "ro",
        "Nu s-a făcut nimic în document: Nora nu a putut acționa la timp. Reluați cererea dacă mai aveți nevoie.",
        "Nu totul a fost făcut: o parte nu a putut fi executată în document la timp.",
    ),
    (
        "tr",
        "Belgede hiçbir şey yapılmadı: Nora zamanında belgede işlem yapamadı. Hâlâ gerekiyorsa lütfen yeniden isteyin.",
        "Bunların tamamı yapılmadı: bir kısmı belgede zamanında çalıştırılamadı.",
    ),
    (
        "vi",
        "Không có gì được thực hiện trong tài liệu: Nora không thể thao tác kịp thời. Nếu vẫn cần, vui lòng yêu cầu lại.",
        "Không phải tất cả đều đã được thực hiện: một phần không thể chạy trong tài liệu kịp thời.",
    ),
    (
        "ja",
        "ドキュメントには何も反映されていません: Nora が時間内に操作できませんでした。必要な場合は、もう一度お試しください。",
        "すべてが完了したわけではありません: 一部は時間内にドキュメントで実行できませんでした。",
    ),
    (
        "ko",
        "문서에 아무 작업도 반영되지 않았습니다: Nora가 제시간에 문서에서 실행하지 못했습니다. 여전히 필요하면 다시 요청해 주세요.",
        "모든 작업이 완료된 것은 아닙니다: 일부는 제시간에 문서에서 실행되지 못했습니다.",
    ),
    ("zh-hans", "文档中未执行任何操作：Nora 未能及时在文档中操作。如仍需要，请重新提出请求。", "并非全部完成：部分操作未能及时在文档中执行。"),
    ("zh-hant", "文件中未執行任何操作：Nora 未能及時在文件中操作。如仍需要，請重新提出請求。", "並非全部完成：部分操作未能及時在文件中執行。"),
];

/// The words of Nora's button in the title bar, which opens and closes her pane (craft-libs `LauncherWords`): its label,
/// and its bubble, as NeoMail says them.
const BUTTON: (&str, &str) = ("Ask Nora", "Nora, the Neoffice assistant");

/// [`BUTTON`] in every other language of the interface (translated by Haiku, 27.10.2026, after NeoMail's French, German,
/// Italian and Spanish): the code, the label, the bubble. French is in [`FRENCH`].
const BUTTON_WORDS: &[(&str, &str, &str)] = &[
    ("de", "Nora fragen", "Nora, die Neoffice-Assistentin"),
    ("it", "Chiedi a Nora", "Nora, l'assistente Neoffice"),
    ("es", "Preguntar a Nora", "Nora, la asistente de Neoffice"),
    ("pt-br", "Perguntar à Nora", "Nora, a assistente Neoffice"),
    ("nl", "Nora vragen", "Nora, de Neoffice-assistente"),
    ("da", "Spørg Nora", "Nora, Neoffice-assistenten"),
    ("sv", "Fråga Nora", "Nora, Neoffice-assistenten"),
    ("pl", "Zapytaj Norę", "Nora, asystentka Neoffice"),
    ("cs", "Zeptat se Nory", "Nora, asistentka Neoffice"),
    ("sl", "Vprašajte Noro", "Nora, asistentka Neoffice"),
    ("hr", "Pitajte Noru", "Nora, asistentica Neoffice"),
    ("sr", "Питајте Nora", "Nora, асистенткиња Neoffice"),
    ("sr-latn", "Pitajte Noru", "Nora, asistentkinja Neoffice"),
    ("bg", "Попитайте Nora", "Nora, асистентката на Neoffice"),
    ("ru", "Спросить Nora", "Nora, ассистентка Neoffice"),
    ("uk", "Запитати Nora", "Nora, асистентка Neoffice"),
    ("et", "Küsi Noralt", "Nora, Neoffice'i assistent"),
    ("hu", "Kérdezze Norát", "Nora, a Neoffice asszisztense"),
    ("ro", "Întrebați-o pe Nora", "Nora, asistenta Neoffice"),
    ("tr", "Nora'ya sorun", "Nora, Neoffice asistanı"),
    ("vi", "Hỏi Nora", "Nora, trợ lý của Neoffice"),
    ("ja", "Noraに質問", "Nora、Neoffice のアシスタント"),
    ("ko", "Nora에게 문의", "Nora, Neoffice 어시스턴트"),
    ("zh-hans", "询问 Nora", "Nora，Neoffice 助手"),
    ("zh-hant", "詢問 Nora", "Nora，Neoffice 助理"),
];

/// `s` in `lang`, when Neoffice has translated it.
pub fn plain(lang: Lang, s: &str) -> Option<&'static str> {
    let code = lang.code();
    if code == "fr" {
        return FRENCH.iter().find(|(english, _)| *english == s).map(|(_, french)| *french);
    }
    let complete = COMPLETE_CATALOGS
        .iter()
        .find(|(language, _)| *language == code)
        .and_then(|(_, table)| table.iter().find(|(english, _)| *english == s))
        .map(|(_, translated)| *translated);
    complete
        .or_else(|| pair(RESUME_WORDS, RESUME, code, s))
        .or_else(|| pair(CORRECTION_WORDS, CORRECTION, code, s))
        .or_else(|| pair(BUTTON_WORDS, BUTTON, code, s))
}

/// The translation of one of `english`'s two strings in `table`, for the language `code`.
fn pair(table: &[(&str, &'static str, &'static str)], english: (&str, &str), code: &str, s: &str) -> Option<&'static str> {
    let words = table.iter().find(|(language, _, _)| *language == code)?;
    if s == english.0 {
        Some(words.1)
    } else if s == english.1 {
        Some(words.2)
    } else {
        None
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
    fn only_french_has_them_all_for_now() {
        let fr = crate::i18n::Lang::all().find(|l| l.code() == "fr");
        assert_eq!(fr.and_then(|l| plain(l, "New conversation")), Some("Nouvelle conversation"));
        assert_eq!(plain(crate::i18n::Lang::EN, "New conversation"), None);
        assert_eq!(fr.and_then(|l| plain(l, "A string upstream has")), None);
        // The four complete catalogs have the two strings the interface looks up, and only those.
        for code in ["ru", "hu", "tr", "bg"] {
            let lang = crate::i18n::Lang::all().find(|l| l.code() == code);
            assert!(lang.and_then(|l| plain(l, "Assistant")).is_some(), "{code}");
            assert!(lang.and_then(|l| plain(l, "Show the Nora assistant pane (experimental)")).is_some(), "{code}");
            assert_eq!(lang.and_then(|l| plain(l, "New conversation")), None, "{code}");
        }
    }

    #[test]
    fn the_words_of_a_request_lost_to_a_tab_in_the_background_are_in_every_language() {
        for lang in crate::i18n::Lang::all().filter(|l| l.code() != "en") {
            let said = plain(lang, RESUME.0).unwrap_or_else(|| panic!("{}: no sentence", lang.code()));
            let resume = plain(lang, RESUME.1).unwrap_or_else(|| panic!("{}: no button", lang.code()));
            assert!(said.contains("Nora") || said.contains("Нора"), "{}: {said:?}", lang.code());
            assert_ne!(said, RESUME.0, "{}", lang.code());
            assert!(!resume.is_empty() && resume.trim() == resume, "{}: {resume:?}", lang.code());
        }
        assert_eq!(plain(crate::i18n::Lang::EN, RESUME.1), None, "English is the source");
        assert_eq!(RESUME_WORDS.len() + 2, crate::i18n::Lang::all().count(), "every language but English and French");
    }

    #[test]
    fn the_words_of_nora_s_button_are_in_every_language() {
        for lang in crate::i18n::Lang::all().filter(|l| l.code() != "en") {
            for english in [BUTTON.0, BUTTON.1] {
                let said = plain(lang, english).unwrap_or_else(|| panic!("{}: no translation of {english:?}", lang.code()));
                assert!(!said.is_empty() && said != english && said.trim() == said, "{}: {said:?}", lang.code());
            }
        }
        assert_eq!(plain(crate::i18n::Lang::EN, BUTTON.0), None, "English is the source");
        assert_eq!(BUTTON_WORDS.len() + 2, crate::i18n::Lang::all().count(), "every language but English and French");
    }

    #[test]
    fn the_words_of_a_reply_that_could_not_act_in_time_are_in_every_language() {
        for lang in crate::i18n::Lang::all().filter(|l| l.code() != "en") {
            for english in [CORRECTION.0, CORRECTION.1] {
                let said = plain(lang, english).unwrap_or_else(|| panic!("{}: no translation of {english:?}", lang.code()));
                assert!(!said.is_empty() && said.trim() == said && said != english, "{}: {said:?}", lang.code());
            }
            assert!(plain(lang, CORRECTION.0).is_some_and(|said| said.contains("Nora") || said.contains("Нора")), "{}", lang.code());
        }
        assert_eq!(CORRECTION_WORDS.len() + 2, crate::i18n::Lang::all().count(), "every language but English and French");
    }

    #[test]
    fn the_strings_of_the_complete_catalogs_are_in_the_french_table_too() {
        for (code, table) in COMPLETE_CATALOGS {
            for (english, translated) in *table {
                assert!(FRENCH.iter().any(|(e, _)| e == english), "{code}: {english:?} has no French entry");
                assert_eq!(translated.trim(), *translated, "{code}: stray whitespace in {translated:?}");
            }
        }
    }
}
