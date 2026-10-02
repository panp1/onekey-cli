use serde::{Deserialize, Serialize};

/// One secret in import, export and `init` payloads.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecretInput {
  pub key: String,
  pub value: String,
}
