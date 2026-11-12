// //// Neoffice — added file (no upstream equivalent): when the page command API may be installed.

//! Upstream's browser build puts `window.wordcraft` on every page (`api.rs`: `execute`, `commands`, `docInspect`, `uiInspect`,
//! `screenshot`), a control channel that any script of the page can call. An editor a page hosts (`?host=parent`: the Drive of the
//! Suite, Neodesk's window) is driven by that page through its bridge alone (`host.rs`), and the page's document must stay the only
//! one and its saves the ones the bridge hands over: no second way in. So the API is installed only in an editor no page hosts.
//! The rule is plain Rust, built for the tests too, and a test reads `web.rs` to make sure the install goes through it.

/// May `window.wordcraft` (the page command API) be installed? Not in an editor a page hosts.
pub fn page_api_allowed(hosted: bool) -> bool {
    !hosted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hosted_editor_has_no_page_command_api() {
        assert!(!page_api_allowed(true), "a hosting page drives the editor through its bridge alone");
        assert!(page_api_allowed(false), "an editor no page hosts keeps upstream's API");
    }

    /// The shell installs the API only through the rule above: the page API is built (`PageApi`) in `api.rs` alone, `api::install` is
    /// called once, in `web.rs`, and a few lines above it stands the rule fed with whether a page hosts the editor. A line added to
    /// the crate that installs it some other way fails here.
    #[test]
    fn the_shell_installs_the_page_api_only_through_the_rule() {
        let sources = [
            ("main.rs", include_str!("main.rs")),
            ("web.rs", include_str!("web.rs")),
            ("host.rs", include_str!("host.rs")),
            ("api.rs", include_str!("api.rs")),
            ("prefs.rs", include_str!("prefs.rs")),
            ("flags.rs", include_str!("flags.rs")),
            ("nora_origin.rs", include_str!("nora_origin.rs")),
            ("save_fallback.rs", include_str!("save_fallback.rs")),
            ("web/font_store.rs", include_str!("web/font_store.rs")),
            ("web/nora.rs", include_str!("web/nora.rs")),
        ];
        let code = |text: &str| -> Vec<String> { text.lines().filter(|l| !l.trim_start().starts_with("//")).map(str::to_string).collect() };
        for (name, text) in &sources {
            let builds = code(text).iter().filter(|l| l.contains("PageApi::new") || l.contains("craft_web_api::page::PageApi")).count();
            assert!(if *name == "api.rs" { builds > 0 } else { builds == 0 }, "{name}: the page API is built in api.rs alone");
        }
        let shell = code(include_str!("web.rs"));
        let installs: Vec<usize> = shell.iter().enumerate().filter(|(_, l)| l.contains("api::install")).map(|(i, _)| i).collect();
        assert_eq!(installs.len(), 1, "{installs:?}");
        let at = installs.first().copied().unwrap_or_default();
        let above = shell.get(at.saturating_sub(4)..=at).unwrap_or_default().join("\n");
        assert!(above.contains("api_policy::page_api_allowed(app_bridge.is_some())"), "the install is not behind the rule: {above}");
    }
}
