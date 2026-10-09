use crate::models::SecretInput;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

/// Protect tokens before YAML parsing, so an unquoted leading `#` is data.
/// Insert secrets only into parsed string values, then serialize them as YAML;
/// secret contents can never create extra fields, documents, or template passes.
///
/// Typing follows YAML quoting: an unquoted placeholder that is the whole value
/// becomes an integer or boolean when the secret is exactly a canonical one
/// (`3`, `-1`, `true`); quoted, block-scalar and embedded placeholders, and any
/// other secret (`00123`, `1e3`, `yes`), stay strings.
pub(super) fn render(
  text: &str,
  entries: &[SecretInput],
) -> Result<String> {
  static TOKENS: OnceLock<Regex> = OnceLock::new();
  let tokens = TOKENS.get_or_init(|| Regex::new(r"#\{\{([^{}\r\n]*)\}\}#").unwrap());
  let mut prefix = "ONEKEYTEMPLATETOKEN".to_owned();
  while text.contains(&prefix) {
    prefix.push('X');
  }
  let mut names = Vec::new();
  let protected = tokens.replace_all(text, |captures: &regex::Captures<'_>| {
    let marker = format!("{prefix}{}END", names.len());
    let start = captures.get(0).unwrap().start();
    names.push(Token {
      name: captures[1].to_owned(),
      may_type: !quoted(text, start) && !in_block_scalar(text, start),
    });
    marker
  });
  let options = serde_saphyr::options! {
    duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
    merge_keys: serde_saphyr::MergeKeyPolicy::Error,
    reject_unsupported_tags: true,
    with_snippet: false,
  };
  // Parser diagnostics can carry literal input values even without snippets.
  // Never attach the underlying error to the user-visible error chain.
  let mut documents: Vec<Value> = serde_saphyr::from_multiple_with_options(&protected, options)
    .map_err(|_| {
      anyhow::anyhow!(
        "invalid YAML template (duplicate keys, merge keys and unsupported tags are refused)"
      )
    })?;
  if documents.is_empty() || text.trim().is_empty() {
    bail!("the YAML template contains no documents");
  }
  let secrets = entries
    .iter()
    .map(|entry| (entry.key.as_str(), entry.value.as_str()))
    .collect::<HashMap<_, _>>();
  for document in &mut documents {
    replace(document, &prefix, &names, &secrets)?;
  }
  let mut output = String::new();
  for (index, document) in documents.iter().enumerate() {
    if index > 0 {
      output.push_str("---\n");
    }
    output.push_str(
      &serde_saphyr::to_string(document)
        .map_err(|_| anyhow::anyhow!("failed to serialize rendered YAML"))?,
    );
  }
  Ok(output)
}

struct Token {
  name: String,
  /// Unquoted and outside a `|`/`>` block, so the value may become a number or boolean.
  may_type: bool,
}

fn quoted(
  text: &str,
  start: usize,
) -> bool {
  matches!(text[..start].chars().next_back(), Some('"' | '\''))
}

/// The token sits in a block scalar when the nearest less-indented line above it
/// opens one (`key: |`, `- >-`, `key: |2+ # note`).
fn in_block_scalar(
  text: &str,
  start: usize,
) -> bool {
  static HEADER: OnceLock<Regex> = OnceLock::new();
  let header = HEADER.get_or_init(|| Regex::new(r"[|>][0-9+-]{0,2}\s*(#.*)?$").unwrap());
  let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
  let line = &text[line_start..start];
  if !line.trim().is_empty() {
    return false;
  }
  let indent = line.len();
  text[..line_start]
    .lines()
    .rev()
    .find(|line| !line.trim().is_empty() && line.len() - line.trim_start().len() < indent)
    .is_some_and(|line| header.is_match(line.trim_end()))
}

/// Canonical integers (fit in i64, no leading zeros or sign tricks) and booleans only.
fn typed(secret: &str) -> Option<Value> {
  static INTEGER: OnceLock<Regex> = OnceLock::new();
  let integer = INTEGER.get_or_init(|| Regex::new(r"^-?(0|[1-9][0-9]*)$").unwrap());
  match secret {
    "true" => Some(Value::Bool(true)),
    "false" => Some(Value::Bool(false)),
    _ if integer.is_match(secret) && secret != "-0" => secret.parse::<i64>().ok().map(Value::from),
    _ => None,
  }
}

fn replace(
  value: &mut Value,
  prefix: &str,
  names: &[Token],
  secrets: &HashMap<&str, &str>,
) -> Result<()> {
  // A whole-value, unquoted placeholder may become a number or boolean.
  if let Value::String(text) = value
    && let Some(index) = text
      .strip_prefix(prefix)
      .and_then(|rest| rest.strip_suffix("END"))
      .and_then(|index| index.parse::<usize>().ok())
    && let Some(token) = names.get(index)
    && token.may_type
    && let Some(typed) = secrets
      .get(token.name.as_str())
      .and_then(|secret| typed(secret))
  {
    *value = typed;
    return Ok(());
  }
  match value {
    Value::String(text) => {
      let mut remaining = text.as_str();
      let mut output = String::new();
      while let Some(start) = remaining.find(prefix) {
        output.push_str(&remaining[..start]);
        let marker = &remaining[start + prefix.len()..];
        let (index, rest) = marker
          .split_once("END")
          .context("invalid YAML template token")?;
        let name = &index
          .parse::<usize>()
          .ok()
          .and_then(|index| names.get(index))
          .context("invalid YAML template token")?
          .name;
        let secret = secrets
          .get(name.as_str())
          .with_context(|| format!("template secret {name:?} not found; child was not started"))?;
        output.push_str(secret);
        remaining = rest;
      }
      output.push_str(remaining);
      *text = output;
    }
    Value::Array(values) => {
      for value in values {
        replace(value, prefix, names, secrets)?;
      }
    }
    Value::Object(values) => {
      if values.keys().any(|key| key.contains(prefix)) {
        bail!("YAML template placeholders are supported in values, not mapping keys");
      }
      for value in values.values_mut() {
        replace(value, prefix, names, secrets)?;
      }
    }
    _ => {}
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  fn entries() -> Vec<SecretInput> {
    vec![
      SecretInput {
        key: "BUILD_NUMBER".into(),
        value: "00123".into(),
      },
      SecretInput {
        key: "DB_PASSWORD".into(),
        value: "'\"\\\n---\ninjected: true\n#{{BUILD_NUMBER}}#".into(),
      },
    ]
  }

  #[test]
  fn quoted_unquoted_inline_and_multidocument_tokens_are_safe_strings() {
    let rendered = render(
      r##"
number: #{{BUILD_NUMBER}}#
image: myapp:#{{BUILD_NUMBER}}#
password: "#{{DB_PASSWORD}}#"
single: '#{{DB_PASSWORD}}#'
block: |-
  #{{DB_PASSWORD}}#
list: [#{{BUILD_NUMBER}}#, '#{{BUILD_NUMBER}}#']
untouched: '${BUILD_NUMBER} {{BUILD_NUMBER}} BUILD_NUMBER'
bool: true
count: 3
nil: null
---
password: #{{DB_PASSWORD}}#
"##,
      &entries(),
    )
    .unwrap();
    let docs: Vec<Value> = serde_saphyr::from_multiple(&rendered).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0]["number"], "00123");
    assert_eq!(docs[0]["image"], "myapp:00123");
    for key in ["password", "single", "block"] {
      assert_eq!(docs[0][key], entries()[1].value);
    }
    assert_eq!(docs[0]["list"], json!(["00123", "00123"]));
    assert_eq!(
      docs[0]["untouched"],
      "${BUILD_NUMBER} {{BUILD_NUMBER}} BUILD_NUMBER"
    );
    assert_eq!(docs[0]["bool"], true);
    assert_eq!(docs[0]["count"], 3);
    assert_eq!(docs[0]["nil"], Value::Null);
    assert_eq!(docs[1]["password"], entries()[1].value);
  }

  #[test]
  fn unquoted_whole_values_take_canonical_integer_and_boolean_types_only() {
    let entries = vec![
      SecretInput {
        key: "REPLICAS".into(),
        value: "3".into(),
      },
      SecretInput {
        key: "NEGATIVE".into(),
        value: "-1".into(),
      },
      SecretInput {
        key: "FLAG".into(),
        value: "true".into(),
      },
      SecretInput {
        key: "PIN".into(),
        value: "00123".into(),
      },
      SecretInput {
        key: "EXP".into(),
        value: "1e3".into(),
      },
      SecretInput {
        key: "YES".into(),
        value: "yes".into(),
      },
      SecretInput {
        key: "HUGE".into(),
        value: "99999999999999999999".into(),
      },
    ];
    let rendered = render(
      r##"
replicas: #{{REPLICAS}}#
negative: #{{NEGATIVE}}#
enabled: #{{FLAG}}#
list: [#{{REPLICAS}}#, "#{{REPLICAS}}#"]
quoted: "#{{REPLICAS}}#"
single: '#{{FLAG}}#'
embedded: v#{{REPLICAS}}#
pin: #{{PIN}}#
exp: #{{EXP}}#
yes: #{{YES}}#
huge: #{{HUGE}}#
block: |-
  #{{REPLICAS}}#
nested:
  folded: >-
    first

    #{{FLAG}}#
  after: #{{REPLICAS}}#
"##,
      &entries,
    )
    .unwrap();
    let doc: Value = serde_saphyr::from_str(&rendered).unwrap();
    assert_eq!(doc["replicas"], json!(3));
    assert_eq!(doc["negative"], json!(-1));
    assert_eq!(doc["enabled"], json!(true));
    assert_eq!(doc["list"], json!([3, "3"]));
    assert_eq!(doc["quoted"], json!("3"));
    assert_eq!(doc["single"], json!("true"));
    assert_eq!(doc["embedded"], json!("v3"));
    assert_eq!(doc["pin"], json!("00123"));
    assert_eq!(doc["exp"], json!("1e3"));
    assert_eq!(doc["yes"], json!("yes"));
    assert_eq!(doc["huge"], json!("99999999999999999999"));
    assert_eq!(doc["block"], json!("3"));
    assert_eq!(doc["nested"]["folded"], json!("first\ntrue"));
    assert_eq!(doc["nested"]["after"], json!(3));
  }

  #[test]
  fn comments_are_not_resolved_and_markers_cannot_collide_with_literal_input() {
    let output = render("# note #{{MISSING}}#\nliteral: ONEKEYTEMPLATETOKEN0END\nvalue: #{{BUILD_NUMBER}}# # note #{{MISSING}}#\n", &entries()).unwrap();
    let doc: Value = serde_saphyr::from_str(&output).unwrap();
    assert_eq!(doc["literal"], "ONEKEYTEMPLATETOKEN0END");
    assert_eq!(doc["value"], "00123");
  }

  #[test]
  fn missing_case_mismatch_invalid_input_and_key_templates_are_refused() {
    for input in [
      "value: #{{MISSING}}#",
      "value: #{{build_number}}#",
      "value: #{{}}#",
      "#{{BUILD_NUMBER}}#: value",
      "value: 1\nvalue: 2",
      "value: [",
      "value: !custom abc",
      "value: {<<: {a: b}}",
      "",
    ] {
      assert!(render(input, &entries()).is_err(), "{input}");
    }
    let error = render("value: !custom literal-secret-marker", &entries()).unwrap_err();
    assert!(!format!("{error:#}").contains("literal-secret-marker"));
  }
}
