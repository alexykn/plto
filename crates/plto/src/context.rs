use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use std::collections::BTreeMap;

pub(crate) type ContextMap = BTreeMap<String, Value>;

#[derive(Serialize, Debug, Clone, Default)]
pub(crate) struct TemplateContext {
    #[serde(flatten)]
    values: ContextMap,
}

impl TemplateContext {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert_string(&mut self, key: &str, value: impl Into<String>) {
        self.values
            .insert(key.to_string(), Value::String(value.into()));
    }

    pub(crate) fn insert_value(&mut self, key: &str, value: Value) {
        self.values.insert(key.to_string(), value);
    }

    pub(crate) fn merge(&mut self, values: ContextMap) {
        self.values.extend(values);
    }

    pub(crate) fn set_dotted(&mut self, key: &str, value: Value) -> Result<()> {
        let parts = key.split('.').collect::<Vec<_>>();
        if parts.iter().any(|part| part.is_empty()) {
            bail!("Invalid context override key {key:?}: dotted key parts must not be empty");
        }
        set_dotted_in_map(&mut self.values, &parts, value)
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub(crate) fn into_value(self) -> Value {
        Value::Object(self.values.into_iter().collect())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ContextOverrides {
    values: ContextMap,
}

impl ContextOverrides {
    pub(crate) fn parse(set_values: &[String], set_string_values: &[String]) -> Result<Self> {
        let mut context = TemplateContext::new();
        for assignment in set_values {
            let (key, raw_value) = split_assignment(assignment)?;
            context.set_dotted(key, infer_value(raw_value)?)?;
        }
        for assignment in set_string_values {
            let (key, raw_value) = split_assignment(assignment)?;
            context.set_dotted(key, Value::String(raw_value.to_string()))?;
        }

        Ok(Self {
            values: context.values,
        })
    }

    pub(crate) fn into_values(self) -> ContextMap {
        self.values
    }
}

fn split_assignment(assignment: &str) -> Result<(&str, &str)> {
    let Some((key, value)) = assignment.split_once('=') else {
        bail!("Invalid context override {assignment:?}: expected key=value");
    };
    if key.is_empty() {
        bail!("Invalid context override {assignment:?}: key must not be empty");
    }
    Ok((key, value))
}

fn infer_value(raw: &str) -> Result<Value> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return parse_array(trimmed);
    }

    Ok(infer_scalar(trimmed))
}

fn parse_array(raw: &str) -> Result<Value> {
    if let Ok(value @ Value::Array(_)) = serde_json::from_str(raw) {
        return Ok(value);
    }

    let inner = &raw[1..raw.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Value::Array(Vec::new()));
    }

    let items = split_array_items(inner)?
        .into_iter()
        .map(|item| Ok(infer_scalar(unquote(item.trim())?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Value::Array(items))
}

fn infer_scalar(raw: &str) -> Value {
    if raw.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if raw.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if is_integer_literal(raw)
        && let Ok(value) = raw.parse::<i64>()
    {
        return Value::Number(Number::from(value));
    }
    if is_float_literal(raw)
        && let Ok(value) = raw.parse::<f64>()
        && let Some(number) = Number::from_f64(value)
    {
        return Value::Number(number);
    }

    Value::String(unquote(raw).unwrap_or(raw).to_string())
}

fn is_integer_literal(raw: &str) -> bool {
    let value = raw.strip_prefix('-').unwrap_or(raw);
    !value.is_empty() && value.chars().all(|char| char.is_ascii_digit())
}

fn is_float_literal(raw: &str) -> bool {
    let value = raw.strip_prefix('-').unwrap_or(raw);
    value.contains('.')
        && value.chars().filter(|char| *char == '.').count() == 1
        && value.chars().any(|char| char.is_ascii_digit())
        && value
            .chars()
            .all(|char| char.is_ascii_digit() || char == '.')
}

fn split_array_items(raw: &str) -> Result<Vec<&str>> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, char) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if char == '\\' {
            escaped = true;
            continue;
        }
        if matches!(char, '"' | '\'') {
            if quote == Some(char) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(char);
            }
            continue;
        }
        if char == ',' && quote.is_none() {
            items.push(raw[start..index].trim());
            start = index + char.len_utf8();
        }
    }

    if quote.is_some() {
        bail!("Invalid array context override: unterminated quoted string");
    }

    items.push(raw[start..].trim());
    Ok(items)
}

fn unquote(raw: &str) -> Result<&str> {
    if raw.len() < 2 {
        return Ok(raw);
    }

    let bytes = raw.as_bytes();
    let quoted = (bytes[0] == b'"' && bytes[raw.len() - 1] == b'"')
        || (bytes[0] == b'\'' && bytes[raw.len() - 1] == b'\'');
    if quoted {
        return Ok(&raw[1..raw.len() - 1]);
    }
    if matches!(bytes[0], b'"' | b'\'') || matches!(bytes[raw.len() - 1], b'"' | b'\'') {
        bail!("Invalid quoted context override value {raw:?}");
    }
    Ok(raw)
}

fn set_dotted_in_map(map: &mut ContextMap, parts: &[&str], value: Value) -> Result<()> {
    let Some((head, tail)) = parts.split_first() else {
        bail!("Invalid empty context override key");
    };

    if tail.is_empty() {
        map.insert((*head).to_string(), value);
        return Ok(());
    }

    let entry = map
        .entry((*head).to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(object) = entry else {
        bail!(
            "Invalid context override key {:?}: parent value is not an object",
            parts.join(".")
        );
    };
    set_dotted_in_object(object, tail, value)
}

fn set_dotted_in_object(
    object: &mut Map<String, Value>,
    parts: &[&str],
    value: Value,
) -> Result<()> {
    let Some((head, tail)) = parts.split_first() else {
        bail!("Invalid empty context override key");
    };

    if tail.is_empty() {
        object.insert((*head).to_string(), value);
        return Ok(());
    }

    let entry = object
        .entry((*head).to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(child) = entry else {
        bail!(
            "Invalid context override key {:?}: parent value is not an object",
            parts.join(".")
        );
    };
    set_dotted_in_object(child, tail, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inferred_scalars() {
        let overrides = ContextOverrides::parse(
            &[
                "port=8000".to_string(),
                "ratio=100.0".to_string(),
                "enabled=True".to_string(),
                "name=api".to_string(),
            ],
            &[],
        )
        .unwrap()
        .into_values();

        assert_eq!(overrides["port"], Value::Number(Number::from(8000)));
        assert_eq!(
            overrides["ratio"],
            Value::Number(Number::from_f64(100.0).unwrap())
        );
        assert_eq!(overrides["enabled"], Value::Bool(true));
        assert_eq!(overrides["name"], Value::String("api".to_string()));
    }

    #[test]
    fn parses_arrays_and_string_overrides() {
        let overrides = ContextOverrides::parse(
            &[
                "features=[auth,metrics]".to_string(),
                "ports=[8000,9000]".to_string(),
            ],
            &["version=1.0".to_string()],
        )
        .unwrap()
        .into_values();

        assert_eq!(
            overrides["features"],
            Value::Array(vec![
                Value::String("auth".to_string()),
                Value::String("metrics".to_string()),
            ])
        );
        assert_eq!(
            overrides["ports"],
            Value::Array(vec![Number::from(8000).into(), Number::from(9000).into()])
        );
        assert_eq!(overrides["version"], Value::String("1.0".to_string()));
    }

    #[test]
    fn parses_dotted_keys() {
        let overrides = ContextOverrides::parse(
            &[
                "author.name=Alex".to_string(),
                "author.email=alex@example.com".to_string(),
            ],
            &[],
        )
        .unwrap()
        .into_values();

        assert_eq!(
            overrides["author"]["name"],
            Value::String("Alex".to_string())
        );
        assert_eq!(
            overrides["author"]["email"],
            Value::String("alex@example.com".to_string())
        );
    }
}
