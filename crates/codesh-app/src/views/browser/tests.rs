use super::{accent_hex, element_clipboard_text};
use codesh_app_core::browser::automation::{ELEMENT_HTML_LIMIT, ELEMENT_TEXT_LIMIT};
use serde_json::json;

#[test]
fn picked_element_copy_includes_context_full_text_and_a_bounded_html_preview() {
    let html = format!("{}OMITTED_TAIL", "é".repeat(ELEMENT_HTML_LIMIT));
    let text = "selected text ".repeat(500);
    let picked = json!({
        "url": "https://example.com/article",
        "selector": "main > p:nth-of-type(3)",
        "bounds": { "x": 4, "y": 8, "width": 120, "height": 24 },
        "html": html,
        "text": text,
        "htmlTruncated": false,
        "textTruncated": false,
    });
    let copied = element_clipboard_text(&picked).expect("valid element selection");
    assert!(copied.contains("Page: https://example.com/article"));
    assert!(!copied.contains("Selector:"));
    assert!(copied.contains(&format!(
        "HTML:\n{}\n… [remaining HTML omitted to save tokens]",
        "é".repeat(ELEMENT_HTML_LIMIT)
    )));
    assert!(!copied.contains("first 500 characters"));
    assert!(!copied.contains("OMITTED_TAIL"));
    assert!(copied.ends_with(&format!("Text:\n{text}")));
    assert!(!copied.contains("Text truncated"));
}

#[test]
fn picked_element_with_no_text_omits_the_text_section() {
    for text in ["", "\n", " \t\r\n", "\u{00a0}"] {
        let copied = element_clipboard_text(&json!({
            "url": "https://www.google.com/",
            "html": "<div><br></div>",
            "text": text,
            "htmlTruncated": false,
            "textTruncated": false,
        }))
        .expect("valid element selection");
        assert_eq!(
            copied,
            "Page: https://www.google.com/\n\nHTML:\n<div><br></div>"
        );
    }
}

#[test]
fn picked_element_marks_html_and_text_source_truncation_separately() {
    let copied = element_clipboard_text(&json!({
        "url": "https://example.com",
        "selector": "#content",
        "html": "<article>…",
        "text": "Visible text",
        "htmlTruncated": true,
        "textTruncated": true,
    }))
    .expect("valid element selection");
    assert!(copied.contains("… [remaining HTML omitted to save tokens]"));
    assert!(copied.ends_with("[Text truncated by Codesh at its 1,000,000-character safety limit]"));
    assert!(element_clipboard_text(&json!({ "text": "missing HTML" })).is_none());
    assert!(element_clipboard_text(&serde_json::Value::Null).is_none());
}

#[test]
fn page_results_cannot_bypass_the_text_safety_limit() {
    let text = "😀".repeat(ELEMENT_TEXT_LIMIT + 1);
    let copied = element_clipboard_text(&json!({
        "url": "https://example.com",
        "html": "<p>…</p>",
        "text": text,
    }))
    .expect("valid selection");
    let (_, selected) = copied.split_once("Text:\n").expect("text section");
    assert_eq!(
        selected
            .split_once("\n\n[Text truncated")
            .expect("limit notice")
            .0
            .chars()
            .count(),
        ELEMENT_TEXT_LIMIT
    );
}

#[test]
fn picker_color_uses_the_theme_accent_as_css_hex() {
    assert_eq!(accent_hex(gpui::rgb(0x12_ab_ef).into()), "#12abef");
}
