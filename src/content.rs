use crate::error::{AppError, ErrorCode};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedContent {
    pub storage_html: String,
    pub markdown_bytes: usize,
    pub storage_html_bytes: usize,
    pub headings: Vec<String>,
}

pub fn markdown_to_storage(markdown: &str) -> Result<ConvertedContent, AppError> {
    reject_unsupported(markdown)?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    Ok(ConvertedContent {
        markdown_bytes: markdown.len(),
        storage_html_bytes: html.len(),
        headings: collect_headings(markdown),
        storage_html: html,
    })
}

fn reject_unsupported(markdown: &str) -> Result<(), AppError> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Image { .. }) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Images and attachments are not supported by the first release.",
                ));
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Raw HTML is not supported by the first release.",
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

fn collect_headings(markdown: &str) -> Vec<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown, options);
    let mut headings = Vec::new();
    let mut current = String::new();
    let mut in_heading = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                current.clear();
            }
            Event::Text(text) | Event::Code(text) if in_heading => {
                current.push_str(&text);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let heading = current.trim();
                if !heading.is_empty() {
                    headings.push(heading.to_string());
                }
                if headings.len() == 5 {
                    break;
                }
            }
            _ => {}
        }
    }

    headings
}
