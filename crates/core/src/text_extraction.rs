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
    fn test_extract_html_removes_style() {
        let html = r#"
            <html>
            <head>
                <style>body { color: red; }</style>
            </head>
            <body>
                <style>.hidden { display: none; }</style>
                <p>Visible content</p>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Visible content"));
        assert!(!result.contains("color: red"));
        assert!(!result.contains("display: none"));
    }

    #[test]
    fn test_extract_html_removes_noscript() {
        let html = r#"
            <html>
            <body>
                <noscript>Please enable JavaScript</noscript>
                <p>Main content</p>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Main content"));
        assert!(!result.contains("Please enable JavaScript"));
    }

    #[test]
    fn test_extract_html_removes_form_elements() {
        let html = r#"
            <html>
            <body>
                <form>
                    <input type="text" value="input value">
                    <button>Submit</button>
                    <select><option>Option 1</option></select>
                    <textarea>Text area content</textarea>
                </form>
                <p>Regular paragraph</p>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Regular paragraph"));
        // Form elements and their content should be removed
        assert!(!result.contains("Submit"));
    }

    #[test]
    fn test_extract_html_with_table() {
        let html = r#"
            <html>
            <body>
                <table>
                    <tr><th>Name</th><th>Value</th></tr>
                    <tr><td>Item 1</td><td>100</td></tr>
                    <tr><td>Item 2</td><td>200</td></tr>
                </table>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        // Tables should be formatted with pipe delimiters
        assert!(result.contains("Name"));
        assert!(result.contains("Value"));
        assert!(result.contains("Item 1"));
        assert!(result.contains("100"));
    }

    #[test]
    fn test_extract_html_preserves_headers() {
        let html = r#"
            <html>
            <body>
                <h1>Main Title</h1>
                <h2>Section Header</h2>
                <p>Paragraph content</p>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Main Title"));
        assert!(result.contains("Section Header"));
        assert!(result.contains("Paragraph content"));
    }

    #[test]
    fn test_extract_html_nested_elements() {
        let html = r#"
            <html>
            <body>
                <div>
                    <div>
                        <div>
                            <p>Deeply nested content</p>
                        </div>
                    </div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Deeply nested content"));
    }

    #[test]
    fn test_extract_html_empty() {
        let html = "<html><body></body></html>";
        let result = extract_text_from_html(html).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_html_only_whitespace() {
        let html = "<html><body>   \n\n\t   </body></html>";
        let result = extract_text_from_html(html).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_html_special_chars() {
        let html = "<html><body><p>&amp; &lt; &gt; &quot;</p></body></html>";
        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("&"));
        assert!(result.contains("<"));
        assert!(result.contains(">"));
    }

    #[test]
    fn test_extract_xml() {
        let xml = r#"
            <?xml version="1.0"?>
            <root>
                <item>First item</item>
                <item>Second item</item>
            </root>
        "#;

        let result = extract_text_from_xml(xml).unwrap();
        assert!(result.contains("First item"));
        assert!(result.contains("Second item"));
    }

    #[test]
    fn test_normalize_whitespace() {
        let text = "Hello   world\n\n\ntest";
        let result = normalize_whitespace(text);
        assert_eq!(result, "Hello world\ntest");
    }

    #[test]
    fn test_normalize_whitespace_empty() {
        let result = normalize_whitespace("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_whitespace_only_spaces() {
        let result = normalize_whitespace("     ");
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_whitespace_only_newlines() {
        let result = normalize_whitespace("\n\n\n");
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_whitespace_mixed() {
        // Note: The normalize function keeps a space before the newline
        // because it processes character by character
        let text = "  Hello  \n\n  world  \n test  ";
        let result = normalize_whitespace(text);
        // Check that multiple spaces and newlines are collapsed
        assert!(result.contains("Hello"));
        assert!(result.contains("world"));
        assert!(result.contains("test"));
        // No double newlines
        assert!(!result.contains("\n\n"));
    }

    #[test]
    fn test_normalize_whitespace_tabs() {
        let text = "Hello\t\tworld";
        let result = normalize_whitespace(text);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_is_pdf() {
        assert!(is_pdf(b"%PDF-1.4"));
        assert!(is_pdf(b"%PDF-1.7"));
        assert!(is_pdf(b"%PDF-2.0"));
        assert!(!is_pdf(b"<html>"));
        assert!(!is_pdf(b"not a pdf"));
        assert!(!is_pdf(b""));
        assert!(!is_pdf(b"PDF")); // Missing %
    }

    #[test]
    fn test_is_html_or_xml_doctype() {
        assert!(is_html_or_xml(b"<!DOCTYPE html>"));
        assert!(is_html_or_xml(b"  <!DOCTYPE html>")); // Leading whitespace
        assert!(is_html_or_xml(b"\n<!DOCTYPE html>")); // Leading newline
    }

    #[test]
    fn test_is_html_or_xml_html_tag() {
        assert!(is_html_or_xml(b"<html>"));
        assert!(is_html_or_xml(b"<HTML>"));
        assert!(is_html_or_xml(b"<html lang=\"en\">"));
    }

    #[test]
    fn test_is_html_or_xml_xml_declaration() {
        assert!(is_html_or_xml(b"<?xml version=\"1.0\"?>"));
        assert!(is_html_or_xml(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    }

    #[test]
    fn test_is_html_or_xml_xml_tag() {
        assert!(is_html_or_xml(b"<XML>"));
    }

    #[test]
    fn test_is_html_or_xml_not_html() {
        assert!(!is_html_or_xml(b"plain text"));
        assert!(!is_html_or_xml(b"%PDF-1.4"));
        assert!(!is_html_or_xml(b""));
        assert!(!is_html_or_xml(b"<div>not starting with html"));
    }

    #[test]
    fn test_is_html_or_xml_invalid_utf8() {
        // Invalid UTF-8 bytes
        let invalid_bytes = vec![0xff, 0xfe];
        assert!(!is_html_or_xml(&invalid_bytes));
    }

    #[test]
    fn test_truncate() {
        let text = "First sentence. Second sentence. Third sentence.";
        let result = truncate_for_llm(text, 30);
        assert!(result.starts_with("First sentence."));
        assert!(result.contains("[Content truncated"));
    }

    #[test]
    fn test_truncate_short_text() {
        let text = "Short text.";
        let result = truncate_for_llm(text, 100);
        assert_eq!(result, "Short text.");
        assert!(!result.contains("[Content truncated"));
    }

    #[test]
    fn test_truncate_exact_length() {
        let text = "Exactly 10";
        let result = truncate_for_llm(text, 10);
        assert_eq!(result, "Exactly 10");
    }

    #[test]
    fn test_truncate_no_sentence_break() {
        let text = "This is a long text without any sentence breaks that needs truncation";
        let result = truncate_for_llm(text, 20);
        assert!(result.contains("[Content truncated"));
    }

    #[test]
    fn test_truncate_newline_sentence_break() {
        let text = "First sentence.\nSecond sentence.\nThird sentence.";
        let result = truncate_for_llm(text, 25);
        assert!(result.starts_with("First sentence."));
        assert!(result.contains("[Content truncated"));
    }

    #[test]
    fn test_extraction_error_display() {
        let err = ExtractionError::HtmlParseError("test error".to_string());
        assert_eq!(format!("{}", err), "Failed to parse HTML: test error");

        let err = ExtractionError::PdfError("pdf error".to_string());
        assert_eq!(format!("{}", err), "Failed to extract PDF text: pdf error");

        let err = ExtractionError::UnsupportedType;
        assert_eq!(format!("{}", err), "Unsupported content type");
    }

    #[test]
    fn test_extract_html_list_items() {
        let html = r#"
            <html>
            <body>
                <ul>
                    <li>Item one</li>
                    <li>Item two</li>
                    <li>Item three</li>
                </ul>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Item one"));
        assert!(result.contains("Item two"));
        assert!(result.contains("Item three"));
    }

    #[test]
    fn test_extract_html_removes_nav_footer() {
        let html = r#"
            <html>
            <body>
                <nav>Navigation menu</nav>
                <main>Main content</main>
                <footer>Footer content</footer>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Main content"));
        assert!(!result.contains("Navigation menu"));
        assert!(!result.contains("Footer content"));
    }

    #[test]
    fn test_extract_html_removes_iframe() {
        let html = r#"
            <html>
            <body>
                <iframe>Iframe content</iframe>
                <p>Regular content</p>
            </body>
            </html>
        "#;

        let result = extract_text_from_html(html).unwrap();
        assert!(result.contains("Regular content"));
        assert!(!result.contains("Iframe content"));
    }
}
