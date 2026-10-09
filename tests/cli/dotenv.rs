use onekey_cli::cli::dotenv::parse;

#[test]
fn parses_without_expansion() {
  let values = parse("A=one\nB=\"two three\"\nEMPTY=\n# ignored\n").unwrap();
  assert_eq!(values.len(), 3);
  assert_eq!(values[1].value, "two three");
  assert_eq!(values[2].value, "");
}

#[test]
fn rejects_duplicates() {
  assert!(parse("A=1\nA=2").is_err());
}

#[test]
fn quoted_values_accept_a_trailing_comment_and_refuse_other_text() {
  let entries = parse("A=\"two words\" # note\nB='single'   # note\nC=\"has # inside\" # note\nD=\"esc \\\" quote\" #x\n").unwrap();
  let pairs: Vec<_> = entries
    .iter()
    .map(|e| (e.key.as_str(), e.value.as_str()))
    .collect();
  assert_eq!(
    pairs,
    [
      ("A", "two words"),
      ("B", "single"),
      ("C", "has # inside"),
      ("D", "esc \" quote")
    ]
  );
  assert!(parse("A=\"open\n").is_err());
  assert!(parse("A=\"x\" trailing\n").is_err());
}
