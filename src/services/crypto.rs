use anyhow::{Result, bail};

/// A master key file holds 32 raw bytes or 64 hex characters (as the server writes it).
pub fn parse_master_key(input: &[u8]) -> Result<Vec<u8>> {
  if input.len() == 32 {
    return Ok(input.to_vec());
  }
  let trimmed = std::str::from_utf8(input).map(|s| s.trim()).unwrap_or("");
  if trimmed.len() == 64
    && let Ok(bytes) = hex::decode(trimmed)
    && bytes.len() == 32
  {
    return Ok(bytes);
  }
  bail!("master key must contain 32 raw bytes or 64-character hex string");
}
