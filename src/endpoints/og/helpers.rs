pub fn escape_html(value: &str) -> String {
  let mut out = String::with_capacity(value.len());
  for c in value.chars() {
    match c {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      '\'' => out.push_str("&#39;"),
      _ => out.push(c),
    }
  }
  out
}

pub fn abbreviate_hash(hash: &str, first_chars: usize, last_chars: usize) -> String {
  let len = hash.chars().count();
  if first_chars + last_chars + 2 >= len {
    return hash.to_string();
  }
  let chars: Vec<char> = hash.chars().collect();
  let first: String = chars[..first_chars].iter().collect();
  let last: String = if last_chars > 0 { chars[len - last_chars..].iter().collect() } else { String::new() };
  format!("{}...{}", first, last)
}

pub fn validate_hive_username(value: &str) -> bool {
  let len = value.len();
  if len < 3 || len > 16 {
    return false;
  }
  for label in value.split('.') {
    if label.len() < 3 {
      return false;
    }
    let bytes = label.as_bytes();
    let first = bytes[0];
    if !first.is_ascii_lowercase() {
      return false;
    }
    let last = bytes[bytes.len() - 1];
    if !(last.is_ascii_lowercase() || last.is_ascii_digit()) {
      return false;
    }
    for &b in bytes {
      if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        return false;
      }
    }
  }
  true
}

pub fn is_hex_string(value: &str, length: usize) -> bool {
  value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn is_cid(value: &str) -> bool {
  value.len() == 59 && value.starts_with("bafyrei")
}

pub fn is_positive_int(value: &str) -> bool {
  if value.is_empty() {
    return false;
  }
  let bytes = value.as_bytes();
  if bytes[0] == b'0' {
    return false;
  }
  bytes.iter().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn escape_basic() {
    assert_eq!(escape_html("a & b"), "a &amp; b");
    assert_eq!(escape_html("<x>"), "&lt;x&gt;");
    assert_eq!(escape_html("\"'"), "&quot;&#39;");
  }

  #[test]
  fn abbreviate() {
    assert_eq!(abbreviate_hash("abcdef1234567890", 4, 4), "abcd...7890");
    assert_eq!(abbreviate_hash("short", 4, 4), "short");
  }

  #[test]
  fn hive_username() {
    assert!(validate_hive_username("alice"));
    assert!(validate_hive_username("alice-1"));
    assert!(validate_hive_username("a.b.c").eq(&false));
    assert!(!validate_hive_username("ab"));
    assert!(!validate_hive_username("1alice"));
    assert!(!validate_hive_username("alice-"));
  }

  #[test]
  fn hex_check() {
    assert!(is_hex_string("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", 40));
    assert!(!is_hex_string("notahex", 40));
  }

  #[test]
  fn cid_check() {
    let sample = "bafyrei".to_string() + &"a".repeat(52);
    assert_eq!(sample.len(), 59);
    assert!(is_cid(&sample));
    assert!(!is_cid("bafyrei"));
    assert!(!is_cid(&("not".to_string() + &"a".repeat(56))));
  }

  #[test]
  fn positive_int_check() {
    assert!(is_positive_int("1"));
    assert!(is_positive_int("123"));
    assert!(!is_positive_int("0"));
    assert!(!is_positive_int(""));
    assert!(!is_positive_int("-1"));
    assert!(!is_positive_int("01"));
  }
}
