//! HTML metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// HTML metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct HtmlMetadata {
    /// File size in bytes
    pub file_size: Option<usize>,
    /// Document title from `<title>`
    pub title: Option<String>,
    /// Content of `meta[name=description]`
    pub meta_description: Option<String>,
    /// `lang` attribute on `<html>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Charset from `meta[charset]` or `meta[http-equiv=Content-Type]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Whether `meta[name=viewport]` is present (mobile-friendly signal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_viewport: Option<bool>,
    /// Number of `<a>` elements
    pub link_count: Option<usize>,
    /// Number of `<link rel="stylesheet">` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stylesheet_count: Option<usize>,
    /// Number of `<script>` elements
    pub script_count: Option<usize>,
    /// Number of `<style>` elements
    pub style_count: Option<usize>,
    /// Number of heading elements (`h1`–`h6`)
    pub heading_count: Option<usize>,
    /// Count per heading level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h1_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h2_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h3_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h4_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h5_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h6_count: Option<usize>,
    /// Number of `<img>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img_count: Option<usize>,
    /// Number of `<table>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_count: Option<usize>,
    /// Number of `<form>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_count: Option<usize>,
    /// Number of `<p>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_count: Option<usize>,
    /// Number of `<ul>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ul_count: Option<usize>,
    /// Number of `<ol>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ol_count: Option<usize>,
    /// Number of `<iframe>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iframe_count: Option<usize>,
    /// Number of `<article>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_count: Option<usize>,
    /// Number of `<nav>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nav_count: Option<usize>,
    /// Number of `<section>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_count: Option<usize>,
    /// Number of `<header>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_count: Option<usize>,
    /// Number of `<footer>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_count: Option<usize>,
    /// Number of `<main>` elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_count: Option<usize>,
    /// Character count of extracted plain text (from p, h1–h6, li, td, th, blockquote, figcaption; excludes script/style)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plain_text_len: Option<usize>,
    /// Word count of extracted plain text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<usize>,
}

impl Serialize for HtmlMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("HtmlMetadata", 32)?;
        crate::serialize_optional!(state, self.file_size, "file_size");
        crate::serialize_optional!(state, self.title, "title");
        crate::serialize_optional!(state, self.meta_description, "meta_description");
        crate::serialize_optional!(state, self.lang, "lang");
        crate::serialize_optional!(state, self.charset, "charset");
        crate::serialize_optional!(state, self.has_viewport, "has_viewport");
        crate::serialize_optional!(state, self.link_count, "link_count");
        crate::serialize_optional!(state, self.stylesheet_count, "stylesheet_count");
        crate::serialize_optional!(state, self.script_count, "script_count");
        crate::serialize_optional!(state, self.style_count, "style_count");
        crate::serialize_optional!(state, self.heading_count, "heading_count");
        crate::serialize_optional!(state, self.h1_count, "h1_count");
        crate::serialize_optional!(state, self.h2_count, "h2_count");
        crate::serialize_optional!(state, self.h3_count, "h3_count");
        crate::serialize_optional!(state, self.h4_count, "h4_count");
        crate::serialize_optional!(state, self.h5_count, "h5_count");
        crate::serialize_optional!(state, self.h6_count, "h6_count");
        crate::serialize_optional!(state, self.img_count, "img_count");
        crate::serialize_optional!(state, self.table_count, "table_count");
        crate::serialize_optional!(state, self.form_count, "form_count");
        crate::serialize_optional!(state, self.p_count, "p_count");
        crate::serialize_optional!(state, self.ul_count, "ul_count");
        crate::serialize_optional!(state, self.ol_count, "ol_count");
        crate::serialize_optional!(state, self.iframe_count, "iframe_count");
        crate::serialize_optional!(state, self.article_count, "article_count");
        crate::serialize_optional!(state, self.nav_count, "nav_count");
        crate::serialize_optional!(state, self.section_count, "section_count");
        crate::serialize_optional!(state, self.header_count, "header_count");
        crate::serialize_optional!(state, self.footer_count, "footer_count");
        crate::serialize_optional!(state, self.main_count, "main_count");
        crate::serialize_optional!(state, self.plain_text_len, "plain_text_len");
        crate::serialize_optional!(state, self.word_count, "word_count");
        state.end()
    }
}

crate::impl_minimal_fallback!(HtmlMetadata, file_size);
