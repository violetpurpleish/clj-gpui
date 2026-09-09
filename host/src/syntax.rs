use gpui_kit::App;
use gpui_kit::component::{
    highlighter::{GrammarConfig, LanguageRegistry},
    input::{
        AutoClosingPair, BracketPair, IndentationRules, SyntaxContext,
        language_config::LanguageConfig, set_language_config,
    },
};
use regex::Regex;

fn register_highlighter() {
    let highlights = format!(
        "{}\n{}",
        tree_sitter_clojure_orchard::HIGHLIGHTS_QUERY,
        include_str!("clojure-highlights.scm")
    );
    LanguageRegistry::singleton().register(
        "clojure",
        &GrammarConfig::new(
            "clojure",
            tree_sitter_clojure_orchard::LANGUAGE.into(),
            vec![],
            &highlights,
            "",
            "",
        ),
    );
}

fn clojure_editing_config() -> LanguageConfig {
    let structural = [
        BracketPair::new("(", ")"),
        BracketPair::new("[", "]"),
        BracketPair::new("{", "}"),
    ];
    LanguageConfig::default()
        .brackets(structural)
        .auto_closing_pairs(
            [
                AutoClosingPair::new("(", ")"),
                AutoClosingPair::new("[", "]"),
                AutoClosingPair::new("{", "}"),
                AutoClosingPair::new("\"", "\""),
            ]
            .into_iter()
            .map(|pair| pair.not_in([SyntaxContext::String, SyntaxContext::Comment])),
        )
        .indentation_rules(IndentationRules::new(
            // Treat the last still-open delimiter of each kind as an indent.
            // This covers ordinary forms such as `(let [x 1]` rather than
            // requiring the line to end immediately after an opener.
            Regex::new(r"(?:\([^)]*|\[[^\]]*|\{[^}]*)$").expect("valid Clojure indent regex"),
            Regex::new(r"^\s*[\)\]\}]").expect("valid Clojure dedent regex"),
        ))
}

pub fn init(cx: &mut App) {
    register_highlighter();
    set_language_config("clojure", clojure_editing_config(), cx);
}

#[cfg(test)]
mod tests {
    #[test]
    fn clojure_highlighter_loads_without_plain_text_fallback() {
        super::register_highlighter();
        let mut highlighter = gpui_kit::component::highlighter::SyntaxHighlighter::new("clojure");
        assert_eq!(highlighter.language().as_ref(), "clojure");
        let source = "(defn hi [] :ok)";
        highlighter.update(None, &source.into(), None);
        assert!(!highlighter.tree().unwrap().root_node().has_error());
        let theme = gpui_kit::component::highlighter::HighlightTheme::default_dark();
        let styles = highlighter.styles(&(0..source.len()), theme.as_ref());
        for token in ["defn", ":ok"] {
            let start = source.find(token).unwrap();
            assert!(
                styles.iter().any(|(range, style)| {
                    range.start == start
                        && range.end == start + token.len()
                        && style.color.is_some()
                }),
                "missing color for {token}"
            );
        }
    }

    #[test]
    fn clojure_editing_rules_keep_reader_quote_literal() {
        use gpui_kit::component::input::SyntaxContext;

        let config = super::clojure_editing_config();
        let pairs = config.auto_closing_pairs.expect("Clojure pairs");
        assert!(pairs.iter().any(|pair| pair.open.as_ref() == "("));
        assert!(pairs.iter().any(|pair| pair.open.as_ref() == "\""));
        assert!(!pairs.iter().any(|pair| pair.open.as_ref() == "'"));
        for pair in &pairs {
            assert!(pair.not_in.contains(&SyntaxContext::String));
            assert!(pair.not_in.contains(&SyntaxContext::Comment));
        }
        let indent = config.indentation_rules.expect("Clojure indentation");
        assert!(
            indent
                .increase_indent_pattern
                .unwrap()
                .is_match("(let [x 1]")
        );
        let decrease = indent.decrease_indent_pattern.unwrap();
        for closing in ["  )", "]", "\t}"] {
            assert!(decrease.is_match(closing), "missing dedent for {closing:?}");
        }
    }
}
