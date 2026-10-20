use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};

/// Renders a post body to HTML. Raw HTML in the Markdown is turned into
/// plain text rather than passed through, and links/images may only point at
/// http(s), mailto or same-site URLs, so a post can never inject markup or
/// scripts into the page — the template marks the result `safe` only because
/// of this.
pub fn to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(CowStr::from(raw.into_string())),
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
        }
        other => other,
    });
    let mut out = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut out, events);
    out
}

/// Keeps http(s), mailto, and relative/anchor URLs; anything else
/// (`javascript:`, `data:`, …) becomes "#".
fn safe_url(url: CowStr<'_>) -> CowStr<'_> {
    let lower = url.trim().to_ascii_lowercase();
    let scheme_end = lower.find(|c: char| c == ':' || c == '/' || c == '?' || c == '#');
    let has_scheme = matches!(scheme_end, Some(i) if lower.as_bytes()[i] == b':');
    if !has_scheme || lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("mailto:") {
        url
    } else {
        CowStr::Borrowed("#")
    }
}

#[cfg(test)]
mod tests {
    use super::to_html;

    #[test]
    fn renders_markdown() {
        let html = to_html("# Title\n\nSome **bold** text and `code`.");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn escapes_raw_html() {
        let html = to_html("Hello <script>alert(1)</script>\n\n<div onclick=\"x()\">hi</div>");
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<div onclick"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn neutralises_unsafe_links() {
        for md in ["[a](javascript:alert(1))", "[a]( JavaScript:alert(1))", "[a](data:text/html,x)", "![i](javascript:x)"] {
            let html = to_html(md);
            assert!(!html.to_lowercase().contains("javascript:"), "{md} -> {html}");
            assert!(!html.contains("data:"), "{md} -> {html}");
        }
        assert!(to_html("[a](https://example.com)").contains("href=\"https://example.com\""));
        assert!(to_html("[a](/blog/x)").contains("href=\"/blog/x\""));
        assert!(to_html("[a](mailto:x@y.z)").contains("href=\"mailto:x@y.z\""));
    }
}
