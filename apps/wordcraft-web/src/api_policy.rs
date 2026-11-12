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

    /// The shell installs the API only through the rule above: a line added to `web.rs` that installs it some other way fails here.
    #[test]
    fn the_shell_installs_the_page_api_only_through_the_rule() {
        let shell = include_str!("web.rs");
        let installs: Vec<&str> = shell.lines().filter(|l| l.contains("api::install(") && !l.trim_start().starts_with("//")).collect();
        assert_eq!(installs.len(), 1, "{installs:?}");
        let at = shell.find("api::install(").unwrap_or_default();
        let before = shell.get(at.saturating_sub(400)..at).unwrap_or_default();
        assert!(before.contains("api_policy::page_api_allowed(app_bridge.is_some())"), "the install is not behind the rule: {before}");
    }
}
