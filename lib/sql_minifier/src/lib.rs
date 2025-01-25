extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{ LitStr, parse_macro_input };
use std::{ env::var, fs::read_to_string };

mod minify_sql {
  pub fn minify(sql: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut sl_comment = false;
    let mut ml_comment_depth: usize = 0;

    let mut i = 0;
    while i < sql.len() {
      let c = sql[i..=i].chars().next().unwrap();

      // Handle string literals (single-quoted)
      if c == '\'' && !sl_comment && ml_comment_depth == 0 {
        in_string = !in_string; // Toggle the string state
        result.push(c);
      } else if !in_string {
        // Handle comments (single-line and multi-line)
        if ml_comment_depth == 0 && i + 1 < sql.len() && &sql[i..i + 2] == "--" {
          sl_comment = true; // Enter single-line comment mode
        } else if i + 1 < sql.len() && &sql[i..i + 2] == "/*" {
          ml_comment_depth += 1; // Enter multi-line comment mode
          i += 1; // Skip the '*' character in '/*'
        } else if sl_comment && c == '\n' {
          // Closing single-line comment
          sl_comment = false;
        } else if ml_comment_depth > 0 && i + 1 < sql.len() && &sql[i..i + 2] == "*/" {
          // Closing multi-line comment
          ml_comment_depth -= 1;
          i += 1; // Skip the '/' character in '*/'
        } else if sl_comment || ml_comment_depth > 0 {
          // Skip characters inside a comment
          // Do nothing with characters inside comments
        } else if c.is_whitespace() {
          // Normal character, skip spaces and newlines if not inside string or comment
          if !result.is_empty() && !result.ends_with(' ') {
            result.push(' '); // Add a single space between non-space characters
          }
        } else {
          result.push(c); // Add non-space characters
        }
      } else {
        // Inside a string, just add the character (don't minify)
        result.push(c);
      }

      i += 1;
    }

    // Remove trailing space if it exists
    if result.ends_with(' ') {
      result.pop();
    }

    // Remove unnecessary spaces
    let trim_chars: Vec<&str> = vec![",", ";", "(", ")", ">", "<", ">=", "<=", "!=", "<>", "=", "+", "-", "*", "/"];
    for &c in &trim_chars {
      result = result.replace(&format!(" {}", c), c);
      result = result.replace(&format!("{} ", c), c);
    }

    // Remove unnecessary trailing semicolon
    if result.ends_with(';') {
      result.pop();
    }

    result
  }
}

#[proc_macro]
pub fn minify_sql_file(input: TokenStream) -> TokenStream {
  let path = format!("{}/{}", var("CARGO_MANIFEST_DIR").unwrap(), parse_macro_input!(input as LitStr).value());
  let contents = read_to_string(path).expect("Could not read SQL file");
  let minified = minify_sql::minify(contents.as_str());
  TokenStream::from(quote! { #minified })
}
