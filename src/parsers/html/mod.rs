//! HTML file metadata extraction

use crate::engine::config::Config;
use crate::parsers::ParseResult;
use crate::results::HtmlMetadata;
use anyhow::Result;
use scraper::{Html, Selector};

fn count(document: &Html, selector: &str) -> Option<usize> {
    Selector::parse(selector)
        .ok()
        .map(|sel| document.select(&sel).count())
}

/// Extract plain text from content-bearing elements (excludes script, style, noscript).
/// Uses: p, h1–h6, li, td, th, blockquote, figcaption.
fn extract_plain_text(document: &Html) -> String {
    const CONTENT: &str = "p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote, figcaption";
    let sel = match Selector::parse(CONTENT) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let parts: Vec<String> = document
        .select(&sel)
        .flat_map(|el| {
            el.text()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .collect();
    parts.join(" ").trim().to_string()
}

/// Extract HTML metadata from document content.
pub fn extract_html_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<HtmlMetadata> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("HTML must be valid UTF-8: {}", e))?;
    let document = Html::parse_document(s);

    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.text().next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let meta_description = Selector::parse("meta[name=description]")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let lang = Selector::parse("html[lang]")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("lang"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let charset = Selector::parse("meta[charset]")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("charset"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let has_viewport = Selector::parse("meta[name=viewport]")
        .ok()
        .map(|sel| document.select(&sel).next().is_some());

    let plain_text = extract_plain_text(&document);
    let plain_text_len = if plain_text.is_empty() {
        None
    } else {
        Some(plain_text.chars().count())
    };
    let word_count = if plain_text.is_empty() {
        None
    } else {
        Some(plain_text.split_whitespace().count())
    };

    Ok(HtmlMetadata {
        file_size: Some(stats.byte_count),
        title,
        meta_description,
        lang,
        charset,
        has_viewport,
        link_count: count(&document, "a"),
        stylesheet_count: count(&document, "link[rel~=stylesheet]"),
        script_count: count(&document, "script"),
        style_count: count(&document, "style"),
        heading_count: count(&document, "h1, h2, h3, h4, h5, h6"),
        h1_count: count(&document, "h1"),
        h2_count: count(&document, "h2"),
        h3_count: count(&document, "h3"),
        h4_count: count(&document, "h4"),
        h5_count: count(&document, "h5"),
        h6_count: count(&document, "h6"),
        img_count: count(&document, "img"),
        table_count: count(&document, "table"),
        form_count: count(&document, "form"),
        p_count: count(&document, "p"),
        ul_count: count(&document, "ul"),
        ol_count: count(&document, "ol"),
        iframe_count: count(&document, "iframe"),
        article_count: count(&document, "article"),
        nav_count: count(&document, "nav"),
        section_count: count(&document, "section"),
        header_count: count(&document, "header"),
        footer_count: count(&document, "footer"),
        main_count: count(&document, "main"),
        plain_text_len,
        word_count,
    })
}

/// Extract templates and writing footprint from HTML by running the plain-text pipeline
/// on the extracted body text (p, h1–h6, li, td, th, blockquote, figcaption; excludes script/style).
pub fn extract_html_templates(
    content: &[u8],
    stats: &crate::parsers::ParseResult,
    config: &Config,
) -> Result<crate::results::MiningResult> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("HTML must be valid UTF-8: {}", e))?;
    let document = Html::parse_document(s);
    let plain_text = extract_plain_text(&document);
    if plain_text.trim().is_empty() {
        return Ok(crate::parsers::traits::empty_mining_result(stats));
    }
    crate::parsers::text::plain_text::extract_text_templates(&plain_text, stats, config)
}
