// //// Neoffice — added file (no upstream equivalent): what a command of Nora's really wrote, read before it stays.

//! The links and fields a change of Nora's left in the document, read once the change is made.
//!
//! The Neoffice server refuses a link to a file and a field that reaches outside the document in what
//! Nora asks for, but it reads only the parameters of a command: a command can leave one it was not given
//! in so many words (a replace in a field's code, a style or a format painted with its link, a building
//! block or a document inserted with its fields). WordCraft writes a link's target and a field's code as
//! they are, and Word opens the link, or runs the field when it updates the fields. So after every change
//! of Nora's the pane reads what the change touched (the body's blocks, and the parts: headers, footers,
//! notes, comments, text boxes), and takes the change back when the document holds more of these than
//! before:
//!
//! - a link (`CharProps::link`) that is no web page or e-mail address ([`craft_nora::links::web_or_mail`])
//!   nor a bookmark of this document (`#name`, without another file's `[`, `]`, `\` or `/`);
//! - a field that reads a file, starts a program, inserts from a template or runs a macro when Word updates
//!   it ([`OUTSIDE_FIELDS`]; a deny list, so that the fields WordCraft's own commands make, citations,
//!   form fields and merge fields among them, stay Nora's to make),
//!   or a `HYPERLINK` field whose target is no such place, read as Word reads its code;
//! - an embedded object (a chart, a diagram) that reads another file or an address, or whose link goes
//!   elsewhere than a web page or an e-mail address.
//!
//! What was already there (the person's own links and fields, moved or not) is counted, not refused: only
//! a change that adds to it is.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use craft_nora::links;
use serde_json::{Map, Value};
use wordcraft_doc::{Block, Document, Part};

/// The fields Word runs against something outside the document: they read a file (`INCLUDETEXT`,
/// `INCLUDEPICTURE`, `INCLUDE`, `IMPORT`, `RD`, `LINK`, `DATA`), insert from the attached template, which
/// may be a file of the network (`AUTOTEXT`, `AUTOTEXTLIST`, `GLOSSARY`), talk to another program (`DDE`,
/// `DDEAUTO`, `LINK`, `EMBED`), query a database (`DATABASE`), run a macro (`MACROBUTTON`) or send codes to
/// the printer (`PRINT`).
const OUTSIDE_FIELDS: &[&str] = &[
    "INCLUDETEXT",
    "INCLUDEPICTURE",
    "INCLUDE",
    "IMPORT",
    "RD",
    "LINK",
    "DATA",
    "AUTOTEXT",
    "AUTOTEXTLIST",
    "GLOSSARY",
    "DDE",
    "DDEAUTO",
    "EMBED",
    "DATABASE",
    "MACROBUTTON",
    "PRINT",
];

/// What a bookmark of this document cannot hold and a place in another file does.
const ANOTHER_FILE: &[char] = &['[', ']', '\\', '/'];

/// A `HYPERLINK` field's switches that take a value: `\l` the bookmark, `\o` the tooltip, `\t` the frame.
const SWITCHES_WITH_A_VALUE: &[&str] = &["\\l", "\\o", "\\t"];

/// The longest target or field code quoted back to Nora.
const MAX_QUOTED_CHARS: usize = 200;

/// The most of them listed in one refusal.
const MAX_LISTED: usize = 5;

/// How deep a part is read: a document's own nesting stops far above.
const MAX_DEPTH: usize = 256;

/// One thing the document holds that leads outside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reach {
    /// What it is, to count it wherever it is: its target, its field code.
    key: String,
    /// What Nora is told it is.
    pub(super) what: String,
    /// Where it is: the document's text, a header, a note…
    pub(super) place: String,
}

/// The document's stories as they were before a change: its blocks and its parts, shared with the
/// document (the blocks are reference-counted: keeping them costs no copy).
pub(super) struct Before {
    body: Vec<Arc<Block>>,
    parts: BTreeMap<u32, Part>,
}

impl Before {
    pub(super) fn of(doc: &Document) -> Before {
        Before { body: doc.body.clone(), parts: doc.parts.clone() }
    }
}

/// What `after` holds that leads outside the document more often than it did before: what a change of
/// Nora's added. Only the blocks and parts the change touched are read first (a block it left alone is
/// the very same); the whole document is counted only when one of them holds something.
pub(super) fn added(before: &Before, after: &Document) -> Vec<Reach> {
    let kept: HashSet<*const Block> = before.body.iter().map(Arc::as_ptr).collect();
    let mut touched = Vec::new();
    for block in after.body.iter().filter(|block| !kept.contains(&Arc::as_ptr(block))) {
        read(block.as_ref(), "the document's text", &mut touched);
    }
    for (id, part) in &after.parts {
        if !before.parts.get(id).is_some_and(|old| same_part(old, part)) {
            read(part, &part_place(part), &mut touched);
        }
    }
    if touched.is_empty() {
        return Vec::new();
    }
    let mut held: BTreeMap<String, usize> = BTreeMap::new();
    for reach in all(&before.body, &before.parts) {
        *held.entry(reach.key).or_default() += 1;
    }
    let mut new = Vec::new();
    for reach in all(&after.body, &after.parts) {
        match held.get_mut(&reach.key) {
            Some(count) if *count > 0 => *count -= 1,
            _ => new.push(reach),
        }
    }
    new
}

/// What Nora is told when her change is taken back: what `what` wrote, and what she may write instead.
pub(super) fn refusal(what: &str, found: &[Reach]) -> String {
    let mut listed: Vec<String> = found.iter().take(MAX_LISTED).map(|reach| format!("{} ({})", reach.what, reach.place)).collect();
    if found.len() > MAX_LISTED {
        listed.push(format!("and {} more", found.len() - MAX_LISTED));
    }
    format!(
        "{what} was undone, nothing of it is left: it wrote {}. A link of yours goes to a web page, an e-mail address or a bookmark of \
this document (#name), never to a file; a field of yours is one of the document's own (PAGE, DATE, TOC…), never one that reads a \
file, starts a program or runs a macro. Tell the person; do not look for another way to write it.",
        listed.join("; ")
    )
}

fn same_part(old: &Part, new: &Part) -> bool {
    old.blocks.len() == new.blocks.len() && old.blocks.iter().zip(&new.blocks).all(|(a, b)| Arc::ptr_eq(a, b))
}

fn part_place(part: &Part) -> String {
    format!("the part {:?}", part.kind)
}

/// Everything the document's stories hold that leads outside it.
fn all(body: &[Arc<Block>], parts: &BTreeMap<u32, Part>) -> Vec<Reach> {
    let mut found = Vec::new();
    for block in body {
        read(block.as_ref(), "the document's text", &mut found);
    }
    for part in parts.values() {
        read(part, &part_place(part), &mut found);
    }
    found
}

/// The links, fields and embedded objects of one story, read in the form WordCraft saves its own files in
/// (serde): a link is a `link` text of the run's properties, a field's code an `instr` text, an embedded
/// object's relationship an object with `external: true` and its `target`. A story that cannot be read so is
/// counted as one reach of its own kind, the same before and after.
/// Each target counts once per block or part: a link is on every run of its text, and a run split in two (a
/// word of the person's linked text made bold) is still the one link.
fn read<T: serde::Serialize>(story: &T, place: &str, found: &mut Vec<Reach>) {
    let mut here = Vec::new();
    match serde_json::to_value(story) {
        Ok(value) => walk(&value, place, &mut here, 0),
        Err(_) => here.push(Reach { key: "unreadable".to_string(), what: "a story that could not be read".to_string(), place: place.to_string() }),
    }
    let mut seen = HashSet::new();
    found.extend(here.into_iter().filter(|reach| seen.insert(reach.key.clone())));
}

fn walk(value: &Value, place: &str, found: &mut Vec<Reach>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            check(map, place, found);
            for item in map.values() {
                walk(item, place, found, depth + 1);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, place, found, depth + 1);
            }
        }
        _ => {}
    }
}

fn check(map: &Map<String, Value>, place: &str, found: &mut Vec<Reach>) {
    let mut push = |key: String, what: String| found.push(Reach { key, what, place: place.to_string() });
    if let Some(Value::String(target)) = map.get("link")
        && !link_stays(target)
    {
        push(format!("link:{}", target.trim()), format!("a link to {}", quoted(target)));
    }
    if let Some(Value::String(code)) = map.get("instr")
        && let Some(what) = field_reaches(code)
    {
        push(format!("field:{}", code.trim()), what);
    }
    if map.get("external") == Some(&Value::Bool(true))
        && let Some(Value::String(target)) = map.get("target")
    {
        let hyperlink = map.get("kind").and_then(Value::as_str).is_some_and(|kind| kind.ends_with("/hyperlink"));
        if !hyperlink || !links::web_or_mail(target) {
            push(format!("object:{}", target.trim()), format!("an embedded object that reads {}", quoted(target)));
        }
    }
}

/// Whether a link stays in the document or goes to a web page or an e-mail address: `#name` is a bookmark
/// of this document unless it names another file or a server. An empty one leads nowhere.
fn link_stays(target: &str) -> bool {
    let target = target.trim_matches([' ', '\t', '\n', '\r']);
    if target.is_empty() {
        return true;
    }
    if links::hides_or_controls(target) {
        return false;
    }
    match target.strip_prefix('#') {
        Some(place) => !place.contains("://") && !place.contains(ANOTHER_FILE),
        None => links::web_or_mail(target),
    }
}

/// What a field reaches outside the document, as Nora is told it, if it reaches anything: a field of
/// [`OUTSIDE_FIELDS`], a `HYPERLINK` whose target does not stay ([`link_stays`]), or a code that hides text.
fn field_reaches(code: &str) -> Option<String> {
    let text = code.trim();
    if text.is_empty() {
        return None;
    }
    if links::hides_or_controls(text) {
        return Some(format!("a field whose code hides text ({})", quoted(text)));
    }
    let name: String = if text.starts_with('=') {
        "=".to_string()
    } else {
        text.chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
    };
    if OUTSIDE_FIELDS.contains(&name.as_str()) {
        return Some(format!("a {name} field, which reaches outside the document ({})", quoted(text)));
    }
    if name != "HYPERLINK" {
        return None;
    }
    let rest = text.get(name.len()..).unwrap_or("");
    hyperlink_targets(rest).into_iter().find(|target| !link_stays(target)).map(|target| format!("a HYPERLINK field to {}", quoted(&target)))
}

/// The targets of a `HYPERLINK` field's code after its name, read as Word reads them: a quoted text (a `\"`
/// inside it a quote) or a bare word; a switch (a backslash and one character, never quoted) is skipped, and so
/// is the value of `\l`, `\o` and `\t`. Word reads the first target; every one is read here.
fn hyperlink_targets(rest: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut skip = false;
    let mut chars = rest.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let Some(&first) = chars.peek() else { break };
        let (token, quoted) = if first == '"' {
            chars.next();
            let mut token = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => match chars.next() {
                        Some('"') => token.push('"'),
                        Some(other) => {
                            token.push('\\');
                            token.push(other);
                        }
                        None => token.push('\\'),
                    },
                    '"' => break,
                    other => token.push(other),
                }
            }
            (token, true)
        } else {
            let mut token = String::new();
            while let Some(c) = chars.next_if(|c| !c.is_whitespace()) {
                token.push(c);
            }
            (token, false)
        };
        if skip {
            skip = false;
            continue;
        }
        if !quoted && is_switch(&token) {
            skip = SWITCHES_WITH_A_VALUE.iter().any(|s| s.eq_ignore_ascii_case(&token));
            continue;
        }
        targets.push(token);
    }
    targets
}

/// A field switch: a backslash and one letter or one of `* @ # !`.
fn is_switch(token: &str) -> bool {
    let mut chars = token.chars();
    chars.next() == Some('\\')
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '*' | '@' | '#' | '!'))
        && chars.next().is_none()
}

/// `text` on one line, cut at [`MAX_QUOTED_CHARS`].
fn quoted(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX_QUOTED_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(MAX_QUOTED_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_stays_when_it_is_a_page_an_address_or_a_bookmark() {
        for target in ["https://example.com", "mailto:info@example.com", "#Annexe", "#_Toc123", "", "  "] {
            assert!(link_stays(target), "{target:?}");
        }
        for target in ["C:\\x.docx", "\\\\srv\\share", "file:///C:/x", "Contrat.docx", "#[Contrat.docx]Annexe", "#x://y", "javascript:x"] {
            assert!(!link_stays(target), "{target:?}");
        }
    }

    #[test]
    fn a_field_reaches_outside_when_word_runs_it_against_a_file_or_a_program() {
        for code in [
            "PAGE",
            " NUMPAGES ",
            "DATE \\@ \"d MMMM yyyy\"",
            "TOC \\o \"1-3\" \\h",
            "= 2+2",
            "HYPERLINK \"https://example.com\"",
            "HYPERLINK \\l \"Annexe\"",
            "ADDIN ZOTERO_ITEM {}",
            "MERGEFIELD Nom",
        ] {
            assert_eq!(field_reaches(code), None, "{code:?}");
        }
        for (code, what) in [
            ("INCLUDETEXT \"C:\\\\x.docx\"", "a INCLUDETEXT field"),
            ("includepicture \"https://example.com/pixel.png\"", "a INCLUDEPICTURE field"),
            ("DDEAUTO c:\\\\windows\\\\system32\\\\cmd.exe \"/k calc\"", "a DDEAUTO field"),
            ("MACROBUTTON Run Click", "a MACROBUTTON field"),
            ("AUTOTEXT Signature", "a AUTOTEXT field"),
            ("HYPERLINK \"C:\\\\Tarifs\\\\x.pdf\"", "a HYPERLINK field to C:\\\\Tarifs\\\\x.pdf"),
            ("HYPERLINK \\o \"tip\" \\\\srv\\x", "a HYPERLINK field to \\\\srv\\x"),
            ("HYPERLINK \"https://example.com\" \"C:\\\\x\"", "a HYPERLINK field to C:\\\\x"),
            ("HYPERLINK \"#[Contrat.docx]Annexe\"", "a HYPERLINK field to #[Contrat.docx]Annexe"),
        ] {
            let found = field_reaches(code).unwrap_or_default();
            assert!(found.starts_with(what), "{code:?}: {found}");
        }
    }

    #[test]
    fn a_hyperlink_field_s_targets_are_read_as_word_reads_them() {
        assert_eq!(hyperlink_targets(" \"https://a.example\" \\o \"a tip\""), vec!["https://a.example".to_string()]);
        assert_eq!(hyperlink_targets(" \\l \"Annexe\""), Vec::<String>::new());
        assert_eq!(hyperlink_targets(" \"say \\\"hi\\\"\""), vec!["say \"hi\"".to_string()]);
        assert_eq!(hyperlink_targets(" \"\\\\srv\\x\""), vec!["\\\\srv\\x".to_string()]);
        assert_eq!(hyperlink_targets(" bare"), vec!["bare".to_string()]);
    }

    #[test]
    fn an_embedded_object_that_reads_outside_is_a_reach() {
        let mut found = Vec::new();
        let object = serde_json::json!({"refs": [
            {"id": "rId1", "kind": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package", "target": "embeddings/x.xlsx", "external": false},
            {"id": "rId2", "kind": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject", "target": "file:///C:/x.xlsx", "external": true},
            {"id": "rId3", "kind": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", "target": "https://example.com", "external": true}
        ]});
        walk(&object, "the document's text", &mut found, 0);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].what, "an embedded object that reads file:///C:/x.xlsx");
    }

    #[test]
    fn the_refusal_says_what_and_where() {
        let found = [Reach { key: "link:C:\\x".into(), what: "a link to C:\\x".into(), place: "the document's text".into() }];
        let text = refusal("`insert.link`", &found);
        assert!(text.starts_with("`insert.link` was undone"), "{text}");
        assert!(text.contains("a link to C:\\x (the document's text)"), "{text}");
    }
}
