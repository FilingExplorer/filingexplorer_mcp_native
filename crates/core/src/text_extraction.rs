//! Document text extraction utilities.
//!
//! Extracts text from various document formats (HTML, XML, PDF)
//! optimized for LLM consumption.

use scraper::{Html, Selector};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Failed to parse HTML: {0}")]
    HtmlParseError(String),

    #[error("Failed to extract PDF text: {0}")]
    PdfError(String),

    #[error("Unsupported content type")]
    UnsupportedType,
}

/// Extract text from HTML content, removing scripts, styles, and other non-content elements.
/// Preserves basic structure with newlines for readability.
pub fn extract_text_from_html(html: &str) -> Result<String, ExtractionError> {
    let document = Html::parse_document(html);

    // Selectors for elements to remove
    let remove_selectors = [
        "script", "style", "noscript", "head", "meta", "link",
        "nav", "footer", "header", "aside", "iframe", "object",
        "embed", "form", "input", "button", "select", "textarea",
    ];

    // Build a set of nodes to skip
    let mut skip_nodes = std::collections::HashSet::new();
    for sel_str in &remove_selectors {
        if let Ok(selector) = Selector::parse(sel_str) {
            for element in document.select(&selector) {
                skip_nodes.insert(element.id());
            }
        }
    }

    let mut text_parts = Vec::new();
    let mut in_table = false;
    let mut table_row = Vec::new();

    // Walk through all text nodes
    for node in document.root_element().descendants() {
        if let Some(element) = node.value().as_element() {
            // Skip removed elements and their children
            if skip_nodes.contains(&node.id()) {
                continue;
            }

            let tag_name = element.name();

            // Handle table structure
            match tag_name {
                "table" => in_table = true,
                "tr" => {
                    if !table_row.is_empty() {
                        text_parts.push(format!("| {} |", table_row.join(" | ")));
                        table_row.clear();
                    }
                }
                "td" | "th" => {
                    // Text will be collected in text node handler
                }
                "p" | "div" | "br" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" => {
                    // Add newline before block elements
                    if !text_parts.is_empty() {
                        let last = text_parts.last().map(|s: &String| s.as_str()).unwrap_or("");
                        if !last.ends_with('\n') {
                            text_parts.push("\n".to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(text) = node.value().as_text() {
            // Check if any ancestor is in skip_nodes
            let should_skip = node
                .ancestors()
                .any(|ancestor| skip_nodes.contains(&ancestor.id()));

            if should_skip {
                continue;
            }

            let trimmed = text.trim();
            if !trimmed.is_empty() {
                if in_table {
                    // Check if we're in a td/th
                    let in_cell = node.ancestors().any(|a| {
                        a.value()
                            .as_element()
                            .map(|e| e.name() == "td" || e.name() == "th")
                            .unwrap_or(false)
                    });
                    if in_cell {
                        table_row.push(trimmed.to_string());
                    }
                } else {
                    text_parts.push(trimmed.to_string());
                }
            }
        }

        // Check for end of table
        if let Some(element) = node.value().as_element() {
            if element.name() == "table" {
                in_table = false;
                if !table_row.is_empty() {
                    text_parts.push(format!("| {} |", table_row.join(" | ")));
                    table_row.clear();
                }
            }
        }
    }

    // Join and normalize whitespace
    let result = text_parts.join(" ");
    let normalized = normalize_whitespace(&result);

    Ok(normalized)
}

/// Extract text from XML content
pub fn extract_text_from_xml(xml: &str) -> Result<String, ExtractionError> {
    // For SEC XML documents, we can use HTML parser which handles XML reasonably well
    extract_text_from_html(xml)
}

/// Extract text from PDF bytes
pub fn extract_text_from_pdf(pdf_bytes: &[u8]) -> Result<String, ExtractionError> {
    // Using pdf-extract crate
    pdf_extract::extract_text_from_mem(pdf_bytes)
        .map_err(|e| ExtractionError::PdfError(e.to_string()))
}

/// Normalize whitespace: collapse multiple spaces/newlines into single space/newline
fn normalize_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_was_whitespace = false;
    let mut last_was_newline = false;

    for ch in text.chars() {
        if ch == '\n' {
            if !last_was_newline {
                result.push('\n');
                last_was_newline = true;
            }
            last_was_whitespace = true;
        } else if ch.is_whitespace() {
            if !last_was_whitespace {
                result.push(' ');
            }
            last_was_whitespace = true;
        } else {
            result.push(ch);
            last_was_whitespace = false;
            last_was_newline = false;
        }
    }

    result.trim().to_string()
}

/// Detect if content is likely PDF based on magic bytes
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF")
}

/// Detect if content is likely HTML/XML
pub fn is_html_or_xml(bytes: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim_start();
        trimmed.starts_with("<!DOCTYPE")
            || trimmed.starts_with("<html")
            || trimmed.starts_with("<HTML")
            || trimmed.starts_with("<?xml")
            || trimmed.starts_with("<XML")
    } else {
        false
    }
}

/// Truncate text to a maximum character count, trying to break at sentence boundaries
pub fn truncate_for_llm(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    // Try to break at a sentence boundary
    let search_window = &text[..max_chars];
    let break_point = search_window
        .rfind(". ")
        .or_else(|| search_window.rfind(".\n"))
        .map(|i| i + 1)
        .unwrap_or(max_chars);

    let truncated = &text[..break_point];
    format!("{}\n\n[Content truncated at {} characters]", truncated, break_point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_html() {
        let html = r#"
            <html>
            <head><title>Test</title></head>
            <body>
                <script>alert('ignore me');</script>
                <p>Hello world!</p>
                <div>This is a test.</div>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Hello world!"));
        assert!(result.contains("This is a test."));
        assert!(!result.contains("alert"));
    }

    #[test]
    fn test_normalize_whitespace() {
        let text = "Hello   world\n\n\ntest";
        let result = normalize_whitespace(text);
        assert_eq!(result, "Hello world\ntest");
    }

    #[test]
    fn test_is_pdf() {
        assert!(is_pdf(b"%PDF-1.4"));
        assert!(!is_pdf(b"<html>"));
    }

    #[test]
    fn test_truncate() {
        let text = "First sentence. Second sentence. Third sentence.";
        let result = truncate_for_llm(text, 30);
        assert!(result.starts_with("First sentence."));
        assert!(result.contains("[Content truncated"));
    }
}
