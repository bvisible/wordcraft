// //// Neoffice — added file (no upstream equivalent)
//! The French dictionary this build carries inside itself.
//!
//! Upstream's French is a Language pack that the app downloads, and a build that runs in a browser can neither read a folder nor
//! download anything: it would not check French at all. The Grammalecte "classique" dictionary (version 7.7, Mozilla Public License
//! 2.0, see `ATTRIBUTION.md`) is therefore compressed (raw DEFLATE, the originals byte for byte) in `assets/spelling/fr/` and handed
//! to `craft-proof` once, when the first session is made ([`install`]). Nothing is inflated or parsed until French text is first
//! checked, so a document with no French in it pays nothing, and every French region (`fr-FR`, `fr-CH`, `fr-BE`, `fr-CA`, `fr-LU`,
//! `fr-MC`) shares the one dictionary.
//!
//! A dictionary in the folder the person set for dictionaries is read instead; the French Language pack and the system's
//! dictionaries come after this one (`craft-proof`'s `embedded` module says why). In a browser there is neither.

use std::sync::Once;

use wordcraft_proof::embedded::{Packing, embed_dictionary};

/// `fr.aff`, raw DEFLATE.
const AFF: &[u8] = include_bytes!("../../../assets/spelling/fr/fr.aff.deflate");
/// `fr.dic`, raw DEFLATE.
const DIC: &[u8] = include_bytes!("../../../assets/spelling/fr/fr.dic.deflate");

/// What the dictionary is, as File › Options names it (a name and a licence: no translation).
pub const LABEL: &str = "Grammalecte 7.7 (MPL 2.0)";

/// Hand the French dictionary to `craft-proof`, once for the process (more calls do nothing).
pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Err(e) = embed_dictionary("fr", LABEL, AFF, DIC, Packing::Deflate) {
            log::error!("French spelling is unavailable: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_proof::{ProofLang, embedded, is_correct_in};

    /// The files are the dictionary's own: the MPL notice opens the `.aff`, and the sizes are those of the originals.
    #[test]
    fn the_embedded_files_are_the_original_ones() {
        let inflate = |data: &[u8]| miniz_oxide::inflate::decompress_to_vec_with_limit(data, 8 << 20).unwrap();
        let (aff, dic) = (inflate(AFF), inflate(DIC));
        let head = String::from_utf8_lossy(aff.get(..140).unwrap_or_default()).into_owned();
        assert!(head.contains("Mozilla Public") && head.contains("v. 2.0"), "{head}");
        assert_eq!((aff.len(), dic.len()), (199_870, 1_236_490));
        assert!(aff.windows(9).any(|w| w == b"FLAG long") && aff.windows(9).any(|w| w == b"FULLSTRIP"));
    }

    /// Installed, French is checked in every region from the bytes alone, and it says which dictionary that is.
    #[test]
    fn every_french_region_is_checked_from_the_embedded_dictionary() {
        install();
        install();
        for tag in ["fr", "fr-FR", "fr-CH", "fr-BE", "fr-CA", "fr-LU", "fr-MC", "fra"] {
            let lang = ProofLang::from_tag(Some(tag));
            assert!(lang.is_proofed(), "{tag}");
            assert!(is_correct_in("septante", lang) && is_correct_in("natel", lang) && is_correct_in("peut-être", lang), "{tag}");
            assert!(!is_correct_in("septnate", lang), "{tag}");
            assert_eq!(embedded::label(lang).as_deref(), Some(LABEL), "{tag}");
            assert_eq!(wordcraft_proof::missing_dictionary(Some(tag)), None, "{tag}");
        }
    }
}
