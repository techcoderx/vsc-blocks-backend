use super::helpers::escape_html;

#[derive(Clone, Debug)]
pub struct MetaTags {
  pub title: String,
  pub description: String,
  pub canonical: String,
  pub og_type: String,
  pub image: String,
  pub noindex: bool,
}

pub fn render_html(meta: &MetaTags) -> String {
  let title = escape_html(&meta.title);
  let description = escape_html(&meta.description);
  let canonical = escape_html(&meta.canonical);
  let og_type = escape_html(&meta.og_type);
  let image = escape_html(&meta.image);
  let robots = if meta.noindex { "noindex,nofollow" } else { "index,follow" };
  format!(
    r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="icon" type="image/svg+xml" href="/img/logo.svg" />
    <meta name="theme-color" content="#ED64A6" />
    <title>{title}</title>
    <meta name="description" content="{description}" />
    <meta name="robots" content="{robots}" />
    <link rel="canonical" href="{canonical}" />
    <meta property="og:site_name" content="Magi Blocks" />
    <meta property="og:title" content="{title}" />
    <meta property="og:description" content="{description}" />
    <meta property="og:type" content="{og_type}" />
    <meta property="og:url" content="{canonical}" />
    <meta property="og:image" content="{image}" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="1200" />
    <meta property="og:image:type" content="image/png" />
    <meta name="twitter:card" content="summary" />
    <meta name="twitter:title" content="{title}" />
    <meta name="twitter:description" content="{description}" />
    <meta name="twitter:image" content="{image}" />
  </head>
  <body>
    <h1>{title}</h1>
    <p>{description}</p>
    <p><a href="{canonical}">View on Magi Blocks</a></p>
  </body>
</html>
"##
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_expected_tags() {
    let html = render_html(
      &(MetaTags {
        title: String::from("Hello | Magi Blocks"),
        description: String::from("A <test>"),
        canonical: String::from("https://vsc.techcoderx.com/tx/abc"),
        og_type: String::from("article"),
        image: String::from("https://vsc.techcoderx.com/img/og-image.png"),
        noindex: false,
      })
    );
    assert!(html.contains("<title>Hello | Magi Blocks</title>"));
    assert!(html.contains(r#"name="description" content="A &lt;test&gt;""#));
    assert!(html.contains(r#"property="og:title" content="Hello | Magi Blocks""#));
    assert!(html.contains(r#"property="og:type" content="article""#));
    assert!(html.contains(r#"property="og:url" content="https://vsc.techcoderx.com/tx/abc""#));
    assert!(html.contains(r#"name="twitter:card" content="summary""#));
    assert!(html.contains(r#"name="robots" content="index,follow""#));
  }

  #[test]
  fn noindex_flag() {
    let html = render_html(
      &(MetaTags {
        title: String::from("Not Found | Magi Blocks"),
        description: String::from("x"),
        canonical: String::from("https://x/y"),
        og_type: String::from("website"),
        image: String::from("https://x/og.png"),
        noindex: true,
      })
    );
    assert!(html.contains(r#"name="robots" content="noindex,nofollow""#));
  }
}
