//! HTML file metadata extraction

use anyhow::Result;
use scraper::{Html, Selector};

use crate::config::RuntimeConfig;
use crate::parsers::{
    ParseResult, text::plain_text::extract_text_templates, traits::empty_mining_result,
};
use crate::results::{HtmlMetadata, MiningResult};

/// Extract text content from the first matching element
macro_rules! extract_text {
    ($doc:expr, $sel:expr) => {
        $doc.select(&$sel)
            .next()
            .and_then(|el| el.text().next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
}

/// Extract attribute value from the first matching element
macro_rules! extract_attr {
    ($doc:expr, $sel:expr, $attr:expr) => {
        $doc.select(&$sel)
            .next()
            .and_then(|el| el.value().attr($attr))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
}

/// Count elements matching a selector, wrapped in Some
macro_rules! count_elements {
    ($doc:expr, $sel:expr) => {
        Some($doc.select(&$sel).count())
    };
}

/// Return None if string is empty, otherwise apply transformation and wrap in Some
macro_rules! some_if_not_empty {
    ($val:expr, $transform:expr) => {
        if $val.is_empty() {
            None
        } else {
            Some($transform(&$val))
        }
    };
}

/// Pre-compiled CSS selectors for HTML parsing
struct HtmlSelectors {
    title: Selector,
    meta_description: Selector,
    html_lang: Selector,
    meta_charset: Selector,
    meta_viewport: Selector,
    link: Selector,
    stylesheet: Selector,
    script: Selector,
    style: Selector,
    all_headings: Selector,
    h1: Selector,
    h2: Selector,
    h3: Selector,
    h4: Selector,
    h5: Selector,
    h6: Selector,
    img: Selector,
    table: Selector,
    form: Selector,
    p: Selector,
    ul: Selector,
    ol: Selector,
    iframe: Selector,
    article: Selector,
    nav: Selector,
    section: Selector,
    header: Selector,
    footer: Selector,
    main: Selector,
    content_elements: Selector,
}

impl HtmlSelectors {
    /// Create a new instance with compiled CSS selectors
    fn new() -> Self {
        Self {
            title: Selector::parse("title").expect("Invalid title selector"),
            meta_description: Selector::parse("meta[name=description]")
                .expect("Invalid meta description selector"),
            html_lang: Selector::parse("html[lang]").expect("Invalid html lang selector"),
            meta_charset: Selector::parse("meta[charset]").expect("Invalid meta charset selector"),
            meta_viewport: Selector::parse("meta[name=viewport]")
                .expect("Invalid meta viewport selector"),
            link: Selector::parse("a").expect("Invalid link selector"),
            stylesheet: Selector::parse("link[rel~=stylesheet]")
                .expect("Invalid stylesheet selector"),
            script: Selector::parse("script").expect("Invalid script selector"),
            style: Selector::parse("style").expect("Invalid style selector"),
            all_headings: Selector::parse("h1, h2, h3, h4, h5, h6")
                .expect("Invalid headings selector"),
            h1: Selector::parse("h1").expect("Invalid h1 selector"),
            h2: Selector::parse("h2").expect("Invalid h2 selector"),
            h3: Selector::parse("h3").expect("Invalid h3 selector"),
            h4: Selector::parse("h4").expect("Invalid h4 selector"),
            h5: Selector::parse("h5").expect("Invalid h5 selector"),
            h6: Selector::parse("h6").expect("Invalid h6 selector"),
            img: Selector::parse("img").expect("Invalid img selector"),
            table: Selector::parse("table").expect("Invalid table selector"),
            form: Selector::parse("form").expect("Invalid form selector"),
            p: Selector::parse("p").expect("Invalid p selector"),
            ul: Selector::parse("ul").expect("Invalid ul selector"),
            ol: Selector::parse("ol").expect("Invalid ol selector"),
            iframe: Selector::parse("iframe").expect("Invalid iframe selector"),
            article: Selector::parse("article").expect("Invalid article selector"),
            nav: Selector::parse("nav").expect("Invalid nav selector"),
            section: Selector::parse("section").expect("Invalid section selector"),
            header: Selector::parse("header").expect("Invalid header selector"),
            footer: Selector::parse("footer").expect("Invalid footer selector"),
            main: Selector::parse("main").expect("Invalid main selector"),
            content_elements: Selector::parse(
                "p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote, figcaption",
            )
            .expect("Invalid content elements selector"),
        }
    }
}

/// Extract plain text from content-bearing elements (excludes script, style, noscript).
/// Uses: p, h1–h6, li, td, th, blockquote, figcaption.
fn extract_plain_text(document: &Html, selectors: &HtmlSelectors) -> String {
    let parts: Vec<String> = document
        .select(&selectors.content_elements)
        .flat_map(|el| {
            el.text()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .collect();
    parts.join(" ").trim().to_string()
}

/// Get cached HTML selectors (compiled once, reused for all HTML files)
fn get_html_selectors() -> &'static HtmlSelectors {
    crate::cached_static!(SELECTORS: HtmlSelectors = HtmlSelectors::new())
}

/// Extract plain text from HTML/XHTML (content-bearing elements only).
/// Used by HTML metadata/templates and by EPUB for spine content documents.
pub fn extract_plain_text_from_html(html_str: &str) -> String {
    let document = Html::parse_document(html_str);
    let selectors = get_html_selectors();
    extract_plain_text(&document, selectors)
}

/// Extract HTML metadata from document content.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when the content is not valid UTF-8.
pub fn extract_html_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<HtmlMetadata> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("HTML must be valid UTF-8: {e}"))?;
    let document = Html::parse_document(s);
    let selectors = get_html_selectors();

    let title = extract_text!(document, selectors.title);
    let meta_description = extract_attr!(document, selectors.meta_description, "content");
    let lang = extract_attr!(document, selectors.html_lang, "lang");
    let charset = extract_attr!(document, selectors.meta_charset, "charset");

    let has_viewport = document
        .select(&selectors.meta_viewport)
        .next()
        .is_some()
        .then_some(true);

    let plain_text = extract_plain_text(&document, selectors);
    let plain_text_len = some_if_not_empty!(plain_text, |s: &String| s.chars().count());
    let word_count = some_if_not_empty!(plain_text, |s: &String| s.split_whitespace().count());

    Ok(HtmlMetadata {
        file_size: Some(stats.byte_count),
        title,
        meta_description,
        lang,
        charset,
        has_viewport,
        link_count: count_elements!(document, selectors.link),
        stylesheet_count: count_elements!(document, selectors.stylesheet),
        script_count: count_elements!(document, selectors.script),
        style_count: count_elements!(document, selectors.style),
        heading_count: count_elements!(document, selectors.all_headings),
        h1_count: count_elements!(document, selectors.h1),
        h2_count: count_elements!(document, selectors.h2),
        h3_count: count_elements!(document, selectors.h3),
        h4_count: count_elements!(document, selectors.h4),
        h5_count: count_elements!(document, selectors.h5),
        h6_count: count_elements!(document, selectors.h6),
        img_count: count_elements!(document, selectors.img),
        table_count: count_elements!(document, selectors.table),
        form_count: count_elements!(document, selectors.form),
        p_count: count_elements!(document, selectors.p),
        ul_count: count_elements!(document, selectors.ul),
        ol_count: count_elements!(document, selectors.ol),
        iframe_count: count_elements!(document, selectors.iframe),
        article_count: count_elements!(document, selectors.article),
        nav_count: count_elements!(document, selectors.nav),
        section_count: count_elements!(document, selectors.section),
        header_count: count_elements!(document, selectors.header),
        footer_count: count_elements!(document, selectors.footer),
        main_count: count_elements!(document, selectors.main),
        plain_text_len,
        word_count,
    })
}

/// Extract templates and writing footprint from HTML by running the plain-text pipeline
/// on the extracted body text (p, h1–h6, li, td, th, blockquote, figcaption; excludes script/style).
///
/// # Errors
///
/// Returns [`anyhow::Error`] when the content is not valid UTF-8, or propagates errors from plain-text template extraction.
pub fn extract_html_templates(
    content: &[u8],
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    let s = std::str::from_utf8(content)
        .map_err(|e| anyhow::anyhow!("HTML must be valid UTF-8: {e}"))?;
    let document = Html::parse_document(s);
    let selectors = get_html_selectors();
    let plain_text = extract_plain_text(&document, selectors);
    if plain_text.trim().is_empty() {
        return Ok(empty_mining_result(stats));
    }
    extract_text_templates(&plain_text, stats, config)
}
