use minijinja::value::{Kwargs, Rest, Value, from_args};
use minijinja::{Error, ErrorKind};
use regex::{Captures, Regex, RegexBuilder};

#[derive(Debug, Clone, Copy, Default)]
struct RegexFlags {
    ignorecase: bool,
    multiline: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct RegexReplaceOptions {
    flags: RegexFlags,
    count: usize,
    mandatory_count: usize,
}

pub(crate) fn register(env: &mut minijinja::Environment<'_>) {
    env.add_filter(
        "regex_replace",
        |value: String, pattern: String, replacement: Option<String>, kwargs: Kwargs| {
            regex_replace(&value, &pattern, replacement.as_deref(), &kwargs)
        },
    );
    env.add_filter(
        "regex_search",
        |value: String, pattern: String, args: Rest<Value>| regex_search(&value, &pattern, &args),
    );
    env.add_filter(
        "regex_findall",
        |value: String, pattern: String, kwargs: Kwargs| regex_findall(&value, &pattern, &kwargs),
    );
    env.add_filter("regex_escape", |value: String| regex_escape(&value));
}

fn regex_replace(
    value: &str,
    pattern: &str,
    replacement: Option<&str>,
    kwargs: &Kwargs,
) -> Result<String, Error> {
    let options = parse_regex_replace_options(kwargs)?;
    kwargs.assert_all_used()?;

    let regex = compile_regex(pattern, options.flags)?;
    let replacement = replacement.unwrap_or_default();
    let (rendered, replacement_count) =
        replace_with_ansible_replacement(&regex, value, replacement, options.count)?;

    if options.mandatory_count != 0 && replacement_count != options.mandatory_count {
        return Err(invalid_operation(format!(
            "regex_replace expected {} replacement(s), but performed {}",
            options.mandatory_count, replacement_count
        )));
    }

    Ok(rendered)
}

fn regex_search(value: &str, pattern: &str, args: &Rest<Value>) -> Result<Value, Error> {
    let (selectors, kwargs): (&[Value], Kwargs) = from_args(args)?;
    let flags = parse_regex_flags(&kwargs)?;
    kwargs.assert_all_used()?;

    let regex = compile_regex(pattern, flags)?;
    let Some(captures) = regex.captures(value) else {
        return Ok(Value::from(""));
    };

    if selectors.is_empty() {
        return Ok(Value::from_serialize(
            capture_text(&captures, 0).unwrap_or_default(),
        ));
    }

    let selected = selectors
        .iter()
        .map(|selector| {
            let selector = value_as_str(selector, "regex_search capture selector")?;
            select_capture(&regex, &captures, &selector)
        })
        .collect::<Result<Vec<_>, _>>()?;

    if selected.len() == 1 {
        Ok(Value::from_serialize(&selected[0]))
    } else {
        Ok(Value::from_serialize(selected))
    }
}

fn regex_findall(value: &str, pattern: &str, kwargs: &Kwargs) -> Result<Value, Error> {
    let flags = parse_regex_flags(kwargs)?;
    kwargs.assert_all_used()?;

    let regex = compile_regex(pattern, flags)?;
    let capture_count = regex.captures_len();

    if capture_count <= 1 {
        let matches = regex
            .find_iter(value)
            .map(|regex_match| regex_match.as_str().to_string())
            .collect::<Vec<_>>();
        return Ok(Value::from_serialize(matches));
    }

    if capture_count == 2 {
        let matches = regex
            .captures_iter(value)
            .map(|captures| capture_text(&captures, 1).unwrap_or_default())
            .collect::<Vec<_>>();
        return Ok(Value::from_serialize(matches));
    }

    let matches = regex
        .captures_iter(value)
        .map(|captures| {
            (1..capture_count)
                .map(|index| capture_text(&captures, index).unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    Ok(Value::from_serialize(matches))
}

fn regex_escape(value: &str) -> String {
    regex::escape(value)
}

fn parse_regex_replace_options(kwargs: &Kwargs) -> Result<RegexReplaceOptions, Error> {
    Ok(RegexReplaceOptions {
        flags: parse_regex_flags(kwargs)?,
        count: kwargs.get::<Option<usize>>("count")?.unwrap_or(0),
        mandatory_count: kwargs.get::<Option<usize>>("mandatory_count")?.unwrap_or(0),
    })
}

fn parse_regex_flags(kwargs: &Kwargs) -> Result<RegexFlags, Error> {
    Ok(RegexFlags {
        ignorecase: kwargs.get::<Option<bool>>("ignorecase")?.unwrap_or(false),
        multiline: kwargs.get::<Option<bool>>("multiline")?.unwrap_or(false),
    })
}

fn compile_regex(pattern: &str, flags: RegexFlags) -> Result<Regex, Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(flags.ignorecase)
        .multi_line(flags.multiline)
        .build()
        .map_err(|error| invalid_operation(format!("invalid regex pattern {pattern:?}: {error}")))
}

fn replace_with_ansible_replacement(
    regex: &Regex,
    value: &str,
    replacement: &str,
    count: usize,
) -> Result<(String, usize), Error> {
    let mut rendered = String::new();
    let mut last_match_end = 0;
    let mut replacement_count = 0;

    for captures in regex.captures_iter(value) {
        if count != 0 && replacement_count >= count {
            break;
        }

        let Some(regex_match) = captures.get(0) else {
            continue;
        };

        rendered.push_str(&value[last_match_end..regex_match.start()]);
        rendered.push_str(&render_ansible_replacement(regex, &captures, replacement)?);
        last_match_end = regex_match.end();
        replacement_count += 1;
    }

    rendered.push_str(&value[last_match_end..]);
    Ok((rendered, replacement_count))
}

fn render_ansible_replacement(
    regex: &Regex,
    captures: &Captures<'_>,
    replacement: &str,
) -> Result<String, Error> {
    let mut rendered = String::new();
    let mut chars = replacement.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(index) = control_capture_index(ch) {
            rendered.push_str(&capture_by_index(captures, index)?);
            continue;
        }

        if ch != '\\' {
            rendered.push(ch);
            continue;
        }

        let Some(next) = chars.next() else {
            rendered.push('\\');
            continue;
        };

        if next.is_ascii_digit() {
            let mut index = next.to_string();
            while let Some(peeked) = chars.peek() {
                if !peeked.is_ascii_digit() {
                    break;
                }
                index.push(*peeked);
                chars.next();
            }
            let index = index.parse::<usize>().map_err(|error| {
                invalid_operation(format!("invalid regex replacement capture index: {error}"))
            })?;
            rendered.push_str(&capture_by_index(captures, index)?);
            continue;
        }

        if next == 'g' && matches!(chars.peek(), Some('<')) {
            chars.next();
            let mut name = String::new();
            for name_char in chars.by_ref() {
                if name_char == '>' {
                    break;
                }
                name.push(name_char);
            }
            if name.is_empty() {
                return Err(invalid_operation("empty regex replacement capture name"));
            }
            rendered.push_str(&capture_by_name_or_index(regex, captures, &name)?);
            continue;
        }

        rendered.push(next);
    }

    Ok(rendered)
}

fn select_capture(regex: &Regex, captures: &Captures<'_>, selector: &str) -> Result<String, Error> {
    if let Some(index) = selector.chars().next().and_then(control_capture_index) {
        return capture_by_index(captures, index);
    }

    let Some(selector) = selector.strip_prefix('\\') else {
        return Err(invalid_operation(format!(
            "regex_search capture selector {selector:?} must use Ansible-style syntax like \\1 or \\g<name>"
        )));
    };

    if let Some(name) = selector
        .strip_prefix("g<")
        .and_then(|value| value.strip_suffix('>'))
    {
        if name.is_empty() {
            return Err(invalid_operation("empty regex_search capture name"));
        }
        return capture_by_name_or_index(regex, captures, name);
    }

    let index = selector.parse::<usize>().map_err(|error| {
        invalid_operation(format!(
            "invalid regex_search capture selector {selector:?}: {error}"
        ))
    })?;
    capture_by_index(captures, index)
}

fn control_capture_index(ch: char) -> Option<usize> {
    if ('\u{1}'..='\u{9}').contains(&ch) {
        return Some(ch as usize);
    }
    None
}

fn capture_by_name_or_index(
    regex: &Regex,
    captures: &Captures<'_>,
    name: &str,
) -> Result<String, Error> {
    if let Ok(index) = name.parse::<usize>() {
        return capture_by_index(captures, index);
    }

    if !regex
        .capture_names()
        .flatten()
        .any(|capture_name| capture_name == name)
    {
        return Err(invalid_operation(format!(
            "regex pattern does not define capture group {name:?}"
        )));
    }

    Ok(captures
        .name(name)
        .map(|regex_match| regex_match.as_str().to_string())
        .unwrap_or_default())
}

fn capture_by_index(captures: &Captures<'_>, index: usize) -> Result<String, Error> {
    if index >= captures.len() {
        return Err(invalid_operation(format!(
            "regex pattern does not define capture group {index}"
        )));
    }
    Ok(capture_text(captures, index).unwrap_or_default())
}

fn capture_text(captures: &Captures<'_>, index: usize) -> Option<String> {
    captures
        .get(index)
        .map(|regex_match| regex_match.as_str().to_string())
}

fn value_as_str(value: &Value, label: &str) -> Result<String, Error> {
    value
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| invalid_operation(format!("{label} must be a string")))
}

fn invalid_operation(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidOperation, message.into())
}

#[cfg(test)]
mod tests {
    use minijinja::context;

    use crate::rendering::new_template_environment;

    fn render(template: &str) -> String {
        new_template_environment()
            .render_str(template, context! {})
            .expect("template should render")
    }

    fn render_error(template: &str) -> String {
        new_template_environment()
            .render_str(template, context! {})
            .expect_err("template should fail")
            .to_string()
    }

    #[test]
    fn regex_replace_removes_prefix() {
        assert_eq!(
            render("{{ 'py3-requests' | regex_replace('^py3-', '') }}"),
            "requests"
        );
    }

    #[test]
    fn regex_replace_preserves_non_matches() {
        assert_eq!(
            render("{{ 'requests' | regex_replace('^py3-', '') }}"),
            "requests"
        );
    }

    #[test]
    fn regex_replace_supports_count() {
        assert_eq!(
            render("{{ 'a1b2c3' | regex_replace('\\d', 'x', count=2) }}"),
            "axbxc3"
        );
    }

    #[test]
    fn regex_replace_supports_ignorecase() {
        assert_eq!(
            render("{{ 'PY3-requests' | regex_replace('^py3-', '', ignorecase=true) }}"),
            "requests"
        );
    }

    #[test]
    fn regex_replace_supports_multiline() {
        assert_eq!(
            render("{{ 'py3-one\\npy3-two' | regex_replace('^py3-', '', multiline=true) }}"),
            "one\ntwo"
        );
    }

    #[test]
    fn regex_replace_supports_numeric_captures() {
        assert_eq!(
            render("{{ 'py3-jinja2' | regex_replace('^py3-(.*)$', '\\\\1') }}"),
            "jinja2"
        );
    }

    #[test]
    fn regex_replace_supports_named_captures() {
        assert_eq!(
            render(
                "{{ 'pkg:requests' | regex_replace('^(?P<kind>[^:]+):(?P<name>.+)$', '\\g<name>') }}"
            ),
            "requests"
        );
    }

    #[test]
    fn regex_replace_errors_on_invalid_pattern() {
        assert!(
            render_error("{{ 'x' | regex_replace('(', '') }}").contains("invalid regex pattern")
        );
    }

    #[test]
    fn regex_replace_errors_on_unknown_capture() {
        assert!(
            render_error("{{ 'py3-requests' | regex_replace('^py3-(.*)$', '\\2') }}")
                .contains("does not define capture group 2")
        );
    }

    #[test]
    fn regex_replace_errors_on_mandatory_count_mismatch() {
        assert!(
            render_error("{{ 'requests' | regex_replace('^py3-', '', mandatory_count=1) }}")
                .contains("expected 1 replacement")
        );
    }

    #[test]
    fn regex_search_returns_first_match() {
        assert_eq!(render("{{ 'foo123bar' | regex_search('\\d+') }}"), "123");
    }

    #[test]
    fn regex_search_returns_empty_for_no_match() {
        assert_eq!(render("x{{ 'foobar' | regex_search('\\d+') }}y"), "xy");
    }

    #[test]
    fn regex_search_returns_numeric_capture() {
        assert_eq!(
            render(
                "{{ 'server1/database42' | regex_search('server(\\d+)/database(\\d+)', '\\\\1') }}"
            ),
            "1"
        );
    }

    #[test]
    fn regex_search_returns_named_capture() {
        assert_eq!(
            render(
                "{{ 'pkg:requests' | regex_search('^(?P<kind>[^:]+):(?P<name>.+)$', '\\g<name>') }}"
            ),
            "requests"
        );
    }

    #[test]
    fn regex_search_returns_multiple_captures() {
        assert_eq!(
            render(
                "{{ 'server1/database42' | regex_search('server(\\d+)/database(\\d+)', '\\\\1', '\\\\2') | join(',') }}"
            ),
            "1,42"
        );
    }

    #[test]
    fn regex_search_supports_flags() {
        assert_eq!(
            render("{{ 'PY3-requests' | regex_search('^py3-(.*)$', '\\\\1', ignorecase=true) }}"),
            "requests"
        );
    }

    #[test]
    fn regex_findall_returns_full_matches() {
        assert_eq!(
            render("{{ 'foo123bar456' | regex_findall('\\d+') | join(',') }}"),
            "123,456"
        );
    }

    #[test]
    fn regex_findall_returns_single_capture_matches() {
        assert_eq!(
            render("{{ 'py3-requests py3-jinja2' | regex_findall('py3-([\\w-]+)') | join(',') }}"),
            "requests,jinja2"
        );
    }

    #[test]
    fn regex_findall_returns_multiple_capture_groups() {
        assert_eq!(
            render(
                "{% for pair in 'a1 b2' | regex_findall('([a-z])(\\d)') %}{{ pair | join(':') }};{% endfor %}"
            ),
            "a:1;b:2;"
        );
    }

    #[test]
    fn regex_findall_supports_flags() {
        assert_eq!(
            render(
                "{{ 'PY3-requests py3-jinja2' | regex_findall('py3-([\\w-]+)', ignorecase=true) | join(',') }}"
            ),
            "requests,jinja2"
        );
    }

    #[test]
    fn regex_escape_escapes_metacharacters() {
        assert_eq!(render("{{ 'a+b.txt' | regex_escape }}"), "a\\+b\\.txt");
    }

    #[test]
    fn unknown_kwargs_fail() {
        assert!(
            render_error("{{ 'x' | regex_findall('x', typo=true) }}")
                .contains("unknown keyword argument 'typo'")
        );
    }
}
