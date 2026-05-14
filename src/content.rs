use crate::error::{AppError, ErrorCode};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedContent {
    pub storage_html: String,
    pub markdown_bytes: usize,
    pub source_bytes: usize,
    pub source_representation: &'static str,
    pub storage_html_bytes: usize,
    pub headings: Vec<String>,
}

pub fn markdown_to_storage(markdown: &str) -> Result<ConvertedContent, AppError> {
    reject_unsupported(markdown)?;

    let parser = Parser::new_ext(markdown, parser_options());
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    Ok(ConvertedContent {
        markdown_bytes: markdown.len(),
        source_bytes: markdown.len(),
        source_representation: "markdown",
        storage_html_bytes: html.len(),
        headings: collect_headings(markdown),
        storage_html: html,
    })
}

pub fn storage_to_storage(storage: &str) -> Result<ConvertedContent, AppError> {
    Ok(ConvertedContent {
        storage_html: storage.to_string(),
        markdown_bytes: 0,
        source_bytes: storage.len(),
        source_representation: "storage",
        storage_html_bytes: storage.len(),
        headings: Vec::new(),
    })
}

fn parser_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options
}

fn reject_unsupported(markdown: &str) -> Result<(), AppError> {
    for event in Parser::new_ext(markdown, parser_options()) {
        match event {
            Event::Start(Tag::Image { .. }) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Images and attachments are not supported by the first release.",
                ));
            }
            Event::Start(Tag::Link { dest_url, .. }) if !is_safe_link_destination(&dest_url) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Link destination scheme is not supported.",
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

fn is_safe_link_destination(destination: &str) -> bool {
    let trimmed = destination.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lowercase = trimmed.to_ascii_lowercase();
    lowercase.starts_with("http://")
        || lowercase.starts_with("https://")
        || lowercase.starts_with("mailto:")
        || trimmed.starts_with('/')
        || trimmed.starts_with("../")
        || trimmed.starts_with("./")
        || trimmed.starts_with('#')
        || !has_scheme(trimmed)
}

fn has_scheme(destination: &str) -> bool {
    let mut characters = destination.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !first.is_ascii_alphabetic() {
        return false;
    }

    for character in characters {
        match character {
            ':' => return true,
            '/' | '?' | '#' => return false,
            character
                if character.is_ascii_alphanumeric()
                    || character == '+'
                    || character == '-'
                    || character == '.' => {}
            _ => return false,
        }
    }

    false
}

fn collect_headings(markdown: &str) -> Vec<String> {
    let parser = Parser::new_ext(markdown, parser_options());
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
