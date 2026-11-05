// //// Neoffice — added file (no upstream equivalent)
//! The French dictionary the program carries: Grammalecte's "classique" dictionary (version 7.7, by Olivier R. and
//! contributors, Mozilla Public License 2.0), the `fr.aff` and `fr.dic` of LibreOffice's `fr_FR` dictionaries, kept
//! byte for byte in `assets/spelling/fr/` and only compressed (raw DEFLATE) so that the web build, which reads no folder,
//! stays small. `ATTRIBUTION.md` has the source, the licence and the SHA-256 of each original file.
//!
//! It is read like any installed Hunspell dictionary (decoded by its `SET` line, parsed by [`AffixDict`]), on first
//! use: a document with no French in it never decompresses a byte of it.

use crate::affix::AffixDict;

/// `fr.aff`, raw DEFLATE.
const AFF: &[u8] = include_bytes!("../../../../assets/spelling/fr/fr.aff.deflate");
/// `fr.dic`, raw DEFLATE.
const DIC: &[u8] = include_bytes!("../../../../assets/spelling/fr/fr.dic.deflate");
/// The largest either file may inflate to (the originals are 0.2 and 1.2 MB).
const LIMIT: usize = 8 << 20;

/// What the dictionary is, as File › Options names it (a name and a licence: no translation).
pub(crate) const LABEL: &str = "Grammalecte 7.7 (MPL 2.0)";

/// What the dictionary says about itself, kept with the parsed data.
const SOURCE: &str = "Grammalecte French dictionary 7.7 (classique), https://grammalecte.net/, Mozilla Public License 2.0";

/// The built-in French dictionary, parsed.
pub(crate) fn load() -> Result<AffixDict, String> {
    let inflate = |data: &[u8], what: &str| {
        miniz_oxide::inflate::decompress_to_vec_with_limit(data, LIMIT).map_err(|e| format!("built-in French {what}: inflate failed: {:?}", e.status))
    };
    let (aff, dic) = crate::hunspell::decode(&inflate(AFF, ".aff")?, &inflate(DIC, ".dic")?)?;
    AffixDict::from_hunspell(&aff, &dic, &[SOURCE])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files are the dictionary's own: the MPL notice opens the `.aff`, and the sizes are those of the originals.
    #[test]
    fn the_embedded_files_are_the_original_ones() {
        let aff = miniz_oxide::inflate::decompress_to_vec_with_limit(AFF, LIMIT).unwrap();
        let dic = miniz_oxide::inflate::decompress_to_vec_with_limit(DIC, LIMIT).unwrap();
        let head = String::from_utf8_lossy(aff.get(..140).unwrap_or_default()).into_owned();
        assert!(head.contains("Mozilla Public") && head.contains("v. 2.0"), "{head}");
        assert_eq!((aff.len(), dic.len()), (199_870, 1_236_490));
        assert!(aff.windows(9).any(|w| w == b"FLAG long") && aff.windows(9).any(|w| w == b"FULLSTRIP"));
    }

    #[test]
    fn the_embedded_dictionary_loads() {
        let d = load().unwrap();
        assert_eq!(d.len(), 82_791);
        assert!(d.rule_count() > 5000);
        assert_eq!(d.comments(), [SOURCE]);
    }
}
