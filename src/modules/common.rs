//! Input checks shared with the server, kept identical so the CLI rejects what the server would.
use regex::Regex;
use std::{collections::BTreeMap, sync::OnceLock};

/// Field errors keyed by code, as the server reports them.
#[derive(Debug)]
pub struct ValidationError {
  pub errors: BTreeMap<String, String>,
}

pub fn validate_email(value: &str) -> Result<String, ValidationError> {
  static EMAIL: OnceLock<Regex> = OnceLock::new();
  let normalized = value.trim().to_lowercase();
  let valid = EMAIL.get_or_init(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());
  if normalized.len() > 254 || !valid.is_match(&normalized) {
    return Err(ValidationError {
      errors: BTreeMap::from([(
        "EMAIL_INVALID".into(),
        "Enter a valid email address.".into(),
      )]),
    });
  }
  Ok(normalized)
}

pub fn validate_password(value: &str) -> Result<(), ValidationError> {
  let length = value.chars().count();
  let mut errors = BTreeMap::new();
  if length < 12 {
    errors.insert(
      "PASSWORD_TOO_SHORT".into(),
      "Password must contain at least 12 characters.".into(),
    );
  }
  if length > 128 {
    errors.insert(
      "PASSWORD_TOO_LONG".into(),
      "Password must contain at most 128 characters.".into(),
    );
  }
  if errors.is_empty() {
    Ok(())
  } else {
    Err(ValidationError { errors })
  }
}
