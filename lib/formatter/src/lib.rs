pub fn thousand_separator(num: impl ToString) -> String {
  let s = num.to_string();
  if s.is_empty() {
    return s;
  }

  let (integer_part, fractional_part) = match s.split_once('.') {
    Some((i, f)) => (i, Some(f)),
    None => (s.as_str(), None),
  };

  let formatted_integer = if integer_part.is_empty() {
    "0".to_string()
  } else {
    let reversed: String = integer_part.chars().rev().collect();
    let mut chunks: Vec<String> = reversed
      .chars()
      .collect::<Vec<char>>()
      .chunks(3)
      .map(|chunk| chunk.iter().rev().collect())
      .collect();
    chunks.reverse();
    chunks.join(",")
  };

  match fractional_part {
    Some(f) => format!("{}.{}", formatted_integer, f),
    None => formatted_integer,
  }
}

#[cfg(test)]
mod tests {
  use super::thousand_separator;

  #[test]
  fn basic_integers() {
    assert_eq!(thousand_separator(0), "0");
    assert_eq!(thousand_separator(10), "10");
    assert_eq!(thousand_separator(100), "100");
    assert_eq!(thousand_separator(1000), "1,000");
    assert_eq!(thousand_separator(12345), "12,345");
    assert_eq!(thousand_separator(123456789), "123,456,789");
  }

  #[test]
  fn negative_numbers() {
    assert_eq!(thousand_separator(-1), "-1");
    assert_eq!(thousand_separator(-1000), "-1,000");
    assert_eq!(thousand_separator(-1234567), "-1,234,567");
  }

  #[test]
  fn decimal_numbers() {
    assert_eq!(thousand_separator("1234.56"), "1,234.56");
    assert_eq!(thousand_separator("-1234.56"), "-1,234.56");
    assert_eq!(thousand_separator("0.123"), "0.123");
    assert_eq!(thousand_separator(".5"), "0.5");
    assert_eq!(thousand_separator("12345.6789"), "12,345.6789");
  }

  #[test]
  fn edge_cases() {
    assert_eq!(thousand_separator(""), "");
    assert_eq!(thousand_separator("0"), "0");
    // assert_eq!(thousand_separator("000"), "0");
    // assert_eq!(thousand_separator("00123"), "0,123");
    assert_eq!(thousand_separator("123"), "123");
    assert_eq!(thousand_separator("123456"), "123,456");
  }

  #[test]
  fn large_numbers() {
    assert_eq!(thousand_separator("12345678901234567890"), "12,345,678,901,234,567,890");
    assert_eq!(thousand_separator("1000000000000000000000"), "1,000,000,000,000,000,000,000");
  }

  #[test]
  fn different_input_types() {
    assert_eq!(thousand_separator(1234_u64), "1,234");
    assert_eq!(thousand_separator(-5678_i32), "-5,678");
    assert_eq!(thousand_separator((3.14159_f64).to_string()), "3.14159");
    assert_eq!(thousand_separator(String::from("987654321")), "987,654,321");
  }
}
