// //// Neoffice — added file (no upstream equivalent): the page's URL flags, read as parameters of the URL by name.

//! The flags the page passes in its URL (`?webgl`, `?sample`, `?nora`), read by name as parameters of
//! the URL by the browser's `URLSearchParams`, the way the host bridge reads `?host` and `?author`.
//! Upstream looked for the words anywhere in the query, so `?author=webgl-fan` turned WebGL on and
//! `?nora=https://sample.example` opened the sample document.

/// What the page's URL asks for.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// `?webgl`: WebGL2 instead of WebGPU.
    pub webgl: bool,
    /// `?sample`: the sample document instead of a blank one.
    pub sample: bool,
    /// `?nora` or `?nora=<origin>`: the value as given, empty for a bare `?nora` (`nora_origin::from_param`
    /// says where Nora's calls then go).
    pub nora: Option<String>,
    /// `?lang=fr`: the interface language the page asks for.
    pub lang: Option<String>,
    /// `?locale=fr-CH`: the locale of the hosting page (the Drive of the Suite gives the person's language and the
    /// system's country); its language is the interface's when `?lang=` names none.
    pub locale: Option<String>,
}

impl Flags {
    /// The flags of `search`, the page's `location.search`: its parameters named `webgl`, `sample` and
    /// `nora`, with or without a value.
    pub fn of(search: &str) -> Flags {
        Flags {
            webgl: param(search, "webgl").is_some(),
            sample: param(search, "sample").is_some(),
            nora: param(search, "nora"),
            lang: param(search, "lang"),
            locale: param(search, "locale"),
        }
    }

    /// The tags that name the interface language, in their order: `?lang=`, then `?locale=` (`i18n::set_page_language`
    /// takes the first one that has a catalog).
    pub fn language_tags(&self) -> Vec<&str> {
        [self.lang.as_deref(), self.locale.as_deref()].into_iter().flatten().collect()
    }
}

/// The value of the parameter `name` of `search`, as the browser's `URLSearchParams` reads it: `None`
/// when there is no such parameter, `""` for one without a value.
#[cfg(target_arch = "wasm32")]
fn param(search: &str, name: &str) -> Option<String> {
    web_sys::UrlSearchParams::new_with_str(search).ok()?.get(name)
}

/// The same off the browser, for the tests alone (this module is built natively only for them): their
/// plain queries have no escapes to decode.
#[cfg(not(target_arch = "wasm32"))]
fn param(search: &str, name: &str) -> Option<String> {
    search.trim_start_matches('?').split('&').filter(|item| !item.is_empty()).find_map(|item| {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        (key == name).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_is_a_parameter_of_the_url_not_a_word_in_it() {
        // A value that happens to hold a flag's name asks for nothing.
        assert_eq!(Flags::of("?nora=https://sample.example"), Flags { nora: Some("https://sample.example".into()), ..Flags::default() });
        assert_eq!(Flags::of("?host=parent&author=webgl-fan"), Flags::default());
        assert_eq!(Flags::of("?author=sample&nora=1"), Flags { nora: Some("1".into()), ..Flags::default() });
        // The flags themselves, with or without a value, in any order.
        assert_eq!(Flags::of("?webgl"), Flags { webgl: true, ..Flags::default() });
        assert_eq!(Flags::of("?sample"), Flags { sample: true, ..Flags::default() });
        assert_eq!(Flags::of("?host=parent&sample=1&webgl&nora"), Flags { webgl: true, sample: true, nora: Some(String::new()), ..Flags::default() });
        assert_eq!(Flags::of(""), Flags::default());
        assert_eq!(Flags::of("?"), Flags::default());
    }

    /// The interface's language: `?lang=`, then the hosting page's `?locale=` (the Drive's `fr-CH`), then the
    /// browser's; a tag without a catalog gives way to the next.
    #[test]
    fn the_page_names_the_language_before_the_browser() {
        use wordcraft_ui_egui::i18n::{Lang, page_language};
        let fr = Lang::from_code("fr");
        let tags = |search: &str| Flags::of(search).language_tags().into_iter().map(str::to_string).collect::<Vec<_>>();
        let page = |search: &str| page_language(tags(search).iter().map(String::as_str));
        assert_eq!(page("?host=parent&locale=fr-CH"), fr, "the Drive's locale");
        assert_eq!(page("?locale=fr-CH&lang=de").map(Lang::code), Some("de"), "?lang= first");
        assert_eq!(page("?lang=xx&locale=fr-CH"), fr, "a language without a catalog gives way");
        assert_eq!(page("?lang=&locale=it-CH").map(Lang::code), Some("it"), "an empty ?lang= names none");
        assert_eq!(page("?host=parent&author=Lina"), None, "nothing named: the browser's");
        assert_eq!(page("?locale=xx-YY"), None);
    }
}
