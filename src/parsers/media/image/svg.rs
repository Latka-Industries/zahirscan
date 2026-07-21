//! SVG dimension extraction (header-only; no rasterization)

use quick_xml::Reader;
use quick_xml::events::Event;

/// True when `content` looks like SVG markup (optional XML decl / BOM / whitespace).
#[must_use]
pub fn looks_like_svg(content: &[u8]) -> bool {
    let text = std::str::from_utf8(content).unwrap_or("");
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("<svg") || (lower.starts_with("<?xml") && lower.contains("<svg"))
}

/// Parse root `<svg>` `width` / `height`, falling back to `viewBox` size.
///
/// Only absolute lengths (unitless or `px`) are accepted. Percentages and other
/// CSS units are treated as unknown so we do not invent placeholder pixels.
#[must_use]
pub fn extract_dimensions(content: &[u8]) -> Option<(usize, usize)> {
    let text = std::str::from_utf8(content).ok()?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if local != "svg" {
                    buf.clear();
                    continue;
                }

                let mut width_attr = None;
                let mut height_attr = None;
                let mut view_box = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref())
                        .to_ascii_lowercase();
                    let value = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
                    match key.as_str() {
                        "width" => width_attr = Some(value),
                        "height" => height_attr = Some(value),
                        "viewbox" => view_box = Some(value),
                        _ => {}
                    }
                }

                let width = width_attr.as_deref().and_then(parse_svg_length);
                let height = height_attr.as_deref().and_then(parse_svg_length);
                if let (Some(w), Some(h)) = (width, height) {
                    return Some((w, h));
                }

                if let Some((vb_w, vb_h)) = view_box.as_deref().and_then(parse_view_box_size) {
                    return Some((width.unwrap_or(vb_w), height.unwrap_or(vb_h)));
                }

                return None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn parse_svg_length(raw: &str) -> Option<usize> {
    let s = raw.trim();
    if s.is_empty() || s.contains('%') {
        return None;
    }
    let (num_part, unit) = split_length(s);
    match unit {
        "" | "px" => parse_positive_px(num_part),
        _ => None,
    }
}

fn split_length(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_digit() || c == '.' || c == '+' || c == '-'))
        .map_or(s.len(), |(i, _)| i);
    (&s[..end], s[end..].trim())
}

fn parse_positive_px(num: &str) -> Option<usize> {
    let v: f64 = num.parse().ok()?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    // SVG user units are CSS px; round to nearest pixel for metadata.
    let rounded = v.round();
    if !(0.0..=usize::MAX as f64).contains(&rounded) {
        return None;
    }
    usize::try_from(rounded as u64).ok()
}

fn parse_view_box_size(raw: &str) -> Option<(usize, usize)> {
    let parts: Vec<&str> = raw
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 4 {
        return None;
    }
    let width = parse_positive_px(parts[2])?;
    let height = parse_positive_px(parts[3])?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_svg_markup() {
        assert!(looks_like_svg(
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"
        ));
        assert!(looks_like_svg(
            b"<?xml version=\"1.0\"?>\n<svg viewBox=\"0 0 10 10\"/>"
        ));
        assert!(!looks_like_svg(b"\x89PNG\r\n"));
    }

    #[test]
    fn parses_width_height_px() {
        let svg = br#"<svg width="200px" height="100px" xmlns="http://www.w3.org/2000/svg"/>"#;
        assert_eq!(extract_dimensions(svg), Some((200, 100)));
    }

    #[test]
    fn parses_view_box_fallback() {
        let svg = br#"<svg viewBox="0 0 64 32" xmlns="http://www.w3.org/2000/svg"/>"#;
        assert_eq!(extract_dimensions(svg), Some((64, 32)));
    }

    #[test]
    fn rejects_percent_lengths() {
        let svg = br#"<svg width="100%" height="50%" xmlns="http://www.w3.org/2000/svg"/>"#;
        assert_eq!(extract_dimensions(svg), None);
    }
}
