use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

/// Renders a post body to HTML. Raw HTML in the Markdown is turned into
/// plain text rather than passed through, and links/images may only point at
/// http(s), mailto or same-site URLs, so a post can never inject markup or
/// scripts into the page — the template marks the result `safe` only because
/// of this.
///
/// Headings get GitHub-style ids ("## 8. Monitoring & alerting" →
/// `id="8-monitoring--alerting"`), so a table of contents written as
/// `[…](#8-monitoring--alerting)` links to them.
pub fn to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events: Vec<Event> = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(CowStr::from(raw.into_string())),
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
        }
        other => other,
    }).collect();
    let events = with_heading_ids(events);
    let mut out = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut out, events.into_iter());
    out
}

/// Gives every heading without an explicit id one made from its text,
/// numbering repeats ("setup", "setup-1", …) the way GitHub does.
fn with_heading_ids(mut events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for i in 0..events.len() {
        let Event::Start(Tag::Heading { id: None, .. }) = &events[i] else { continue };
        let text: String = events[i + 1..]
            .iter()
            .take_while(|e| !matches!(e, Event::End(TagEnd::Heading(_))))
            .filter_map(|e| match e {
                Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
                _ => None,
            })
            .collect();
        let base = heading_slug(&text);
        let n = seen.entry(base.clone()).or_insert(0);
        let slug = if *n == 0 { base } else { format!("{base}-{n}") };
        *n += 1;
        if let Event::Start(Tag::Heading { id, .. }) = &mut events[i] {
            *id = Some(CowStr::from(slug));
        }
    }
    events
}

/// GitHub's anchor rule: lowercase, drop punctuation except `-` and `_`,
/// turn each space into `-` (so " & " becomes "--").
fn heading_slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
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
        assert!(html.contains(r#"<h1 id="title">Title</h1>"#));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn headings_get_github_style_ids() {
        let html = to_html("## 1. What the AWS engineer reported\n\n## 8. Monitoring & alerting\n\n## Concept: What is OOM (Out Of Memory)?\n\n### Step 1: Add `swap`\n\n## Setup\n\n## Setup");
        assert!(html.contains(r#"<h2 id="1-what-the-aws-engineer-reported">"#), "{html}");
        assert!(html.contains(r#"<h2 id="8-monitoring--alerting">"#), "{html}");
        assert!(html.contains(r#"<h2 id="concept-what-is-oom-out-of-memory">"#), "{html}");
        assert!(html.contains(r#"<h3 id="step-1-add-swap">"#), "{html}");
        assert!(html.contains(r#"<h2 id="setup">"#) && html.contains(r#"<h2 id="setup-1">"#), "{html}");
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
