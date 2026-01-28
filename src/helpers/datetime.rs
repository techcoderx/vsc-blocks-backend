use chrono::prelude::{ DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc };
use std::error::Error;

pub fn parse_date_str(date_str: &str) -> Result<DateTime<Utc>, Box<dyn Error + Send + Sync>> {
  let naive_datetime = NaiveDateTime::new(NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?, NaiveTime::default());
  Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, Utc))
}

pub fn format_date(date: u32, month: u32, year: i32) -> String {
  format!("{}-{}-{}", year, month, date)
}

#[cfg(test)]
mod tests {
  use crate::{ constants::testnet_const, helpers::datetime::* };

  #[test]
  fn test_parse_start_date() {
    // let conf = testnet_const();
    let parsed = parse_date_str(&testnet_const().start_date);
    assert_eq!(parsed.is_ok(), true);
  }
}
