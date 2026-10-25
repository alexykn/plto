use std::fmt::Write as _;

use minijinja::syntax::SyntaxConfig;
use minijinja::value::{Value, ValueKind};
use minijinja::{
    AutoEscape, Environment, Error, HtmlEscape, Output, State, UndefinedBehavior, escape_formatter,
};

use super::filters;

pub(crate) fn new_template_environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_syntax(
        SyntaxConfig::builder()
            .keep_trailing_newline(true)
            .build()
            .expect("default delimiters are valid"),
    );
    env.set_formatter(legacy_formatter);
    filters::register_all(&mut env);
    env
}

/// Output formatter that keeps the plto 1.0 (minijinja 2) text for booleans, `none`
/// and sequences/maps, which minijinja 3 prints Python-style (`True`, `None`, `['a']`).
/// Everything else, including HTML escaping, is delegated to minijinja.
fn legacy_formatter(out: &mut Output, state: &mut State, value: &Value) -> Result<(), Error> {
    let auto_escape = state.auto_escape();
    if matches!(auto_escape, AutoEscape::None | AutoEscape::Html) {
        let text = match value.kind() {
            ValueKind::Bool | ValueKind::None | ValueKind::Seq | ValueKind::Map => {
                Some(legacy_text(value))
            }
            ValueKind::Iterable if value.len().is_some() => Some(legacy_text(value)),
            _ => None,
        };
        if let Some(text) = text {
            let written = if matches!(auto_escape, AutoEscape::Html) {
                write!(out, "{}", HtmlEscape(&text))
            } else {
                out.write_str(&text)
            };
            return written.map_err(Error::from);
        }
    }
    escape_formatter(out, state, value)
}

fn legacy_text(value: &Value) -> String {
    let mut text = String::new();
    write_legacy(&mut text, value);
    text
}

/// Writes `value` the way minijinja 2 printed it: JSON-like lists and maps with
/// Rust-escaped strings, lowercase `true`/`false`/`none`.
fn write_legacy(out: &mut String, value: &Value) {
    match value.kind() {
        ValueKind::Bool => out.push_str(if value.is_true() { "true" } else { "false" }),
        ValueKind::None => out.push_str("none"),
        ValueKind::Undefined => out.push_str("undefined"),
        ValueKind::String => {
            let _ = write!(out, "{:?}", value.as_str().unwrap_or_default());
        }
        ValueKind::Number if !value.is_integer() => match f64::try_from(value.clone()) {
            Ok(number) => {
                let _ = write!(out, "{number:?}");
            }
            Err(_) => out.push_str(&value.to_string()),
        },
        ValueKind::Seq | ValueKind::Iterable => {
            out.push('[');
            for (index, item) in value.try_iter().into_iter().flatten().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_legacy(out, &item);
            }
            out.push(']');
        }
        ValueKind::Map => {
            out.push('{');
            for (index, key) in value.try_iter().into_iter().flatten().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_legacy(out, &key);
                out.push_str(": ");
                write_legacy(out, &value.get_item(&key).unwrap_or(Value::UNDEFINED));
            }
            out.push('}');
        }
        _ => out.push_str(&value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use minijinja::context;

    use super::new_template_environment;

    fn render_named(name: &str, source: &str) -> String {
        new_template_environment()
            .render_named_str(name, source, context! {})
            .expect("template should render")
    }

    fn render(source: &str) -> String {
        render_named("t.txt", source)
    }

    #[test]
    fn bools_render_lowercase() {
        assert_eq!(
            render("{{ true }} {{ false }} {{ 1 == 1 }}"),
            "true false true"
        );
    }

    #[test]
    fn none_renders_lowercase() {
        assert_eq!(render("{{ none }}"), "none");
    }

    #[test]
    fn list_renders_json_style() {
        assert_eq!(
            render("{{ ['a', 1, 2.5, true] }}"),
            r#"["a", 1, 2.5, true]"#
        );
    }

    #[test]
    fn map_renders_json_style() {
        assert_eq!(render("{{ {'a': 1, 'b': 'x'} }}"), r#"{"a": 1, "b": "x"}"#);
    }

    #[test]
    fn nested_containers_render_json_style() {
        assert_eq!(
            render("{{ {'k': [1, ['b', none], {'z': false}]} }} {{ (1, 2) }}"),
            r#"{"k": [1, ["b", none], {"z": false}]} [1, 2]"#
        );
    }

    #[test]
    fn items_filter_renders_pairs_as_lists() {
        assert_eq!(render("{{ {'a': 1} | items }}"), r#"[["a", 1]]"#);
    }

    #[test]
    fn empty_containers_render() {
        assert_eq!(render("{{ [] }} {{ {} }}"), "[] {}");
    }

    #[test]
    fn html_autoescape_escapes_whole_container_text() {
        assert_eq!(
            render_named("t.html", "{{ ['<b>', true] }} {{ true }} {{ '<i>' }}"),
            "[&quot;&lt;b&gt;&quot;, true] true &lt;i&gt;"
        );
    }

    #[test]
    fn safe_strings_bypass_escaping_in_html() {
        assert_eq!(render_named("t.html", "{{ '<b>' | safe }}"), "<b>");
    }

    #[test]
    fn plain_text_templates_are_not_escaped() {
        assert_eq!(render("{{ ['<b>'] }} {{ '<b>' }}"), r#"["<b>"] <b>"#);
    }
}
