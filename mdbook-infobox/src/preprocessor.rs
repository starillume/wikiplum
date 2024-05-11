use anyhow::anyhow;
use lazy_static::lazy_static;
use mdbook::BookItem;
use mdbook::errors::{Result as MdbookResult, Error as MdbookError};
use mdbook::book::Book;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{Options, Parser, Event, Tag};
use regex::{Regex, Captures};
use serde::ser::Impossible;
use std::iter::{Iterator, Peekable};
use std::ops::Range;

pub struct InfoboxPreprocessor;

impl Preprocessor for InfoboxPreprocessor {
    fn name(&self) -> &str {
        "infobox"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> MdbookResult<Book> {
        let mut error = None;
        book.for_each_mut(|section| {
            if error.is_some() {
                return
            }

            if let BookItem::Chapter(ref mut ch) = *section {
                let preprocessed_content = match preprocess_chapter(&ch.content) {
                    Ok(content) => content,
                    Err(e) => {
                        error = Some(e);
                        return;
                    }
                };

                ch.content = preprocessed_content;
            };
        });

        match error {
            Some(e) => Err(e),
            None => Ok(book)
        }
    }
}

fn preprocess_chapter(content: &str) -> MdbookResult<String> {
    let mut output: String = content.into();

    for (infobox_contents, range) in find_infoboxes_contents(content) {
        let infobox = Infobox::from_markdown_content(&infobox_contents)?;
        output.replace_range(range, &infobox.render_html());
    }

    Ok(output)
}

type MarkdownContents = String;

#[derive(Debug, PartialEq, Eq)]
struct Infobox {
    title: String,
    sections: Vec<InfoboxSection>,
}

#[derive(Debug, PartialEq, Eq)]
enum InfoboxSection {
    Image(InfoboxImage),
    Field(InfoboxField),
}

#[derive(Debug, PartialEq, Eq)]
struct InfoboxImage {
    url: String,
    title: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct InfoboxField {
    name: String,
    contents: MarkdownContents,
}

fn find_infoboxes_contents(content: &str) -> Vec<(String, Range<usize>)> {
    lazy_static! {
        static ref RE: Regex = {
            Regex::new(
                r"(?xms)              # insignificant whitespace/multiline/dot matches newline mode
            \{\{\#infobox\}\}      # infobox opening tag
            (.*)                    # infobox contents
            \{\{/infobox\}\}      # infobox closing tag",
            )
            .unwrap()
        };
    };

    RE.captures_iter(content)
        .map(|capture| {
            let full_match = capture.get(0).unwrap();
            let infobox_contents_match = capture.get(1).unwrap();

            (infobox_contents_match.as_str().to_owned(), full_match.range())
        })
        .collect()
}

impl Infobox {
    pub fn from_capture(capture: Captures<'_>) -> MdbookResult<Self> {
        let content = capture.get(1).ok_or(anyhow!("could not find infobox contents"))?;

        todo!()
    }

    pub fn from_markdown_content(content: &str) -> MdbookResult<Self> {
        let mut parser_opts = Options::empty();
        parser_opts.insert(Options::ENABLE_TABLES);
        parser_opts.insert(Options::ENABLE_FOOTNOTES);
        parser_opts.insert(Options::ENABLE_STRIKETHROUGH);
        parser_opts.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(content, parser_opts);
        let mut iter = parser.into_iter().peekable();
        let title = Self::parse_infobox_title(&mut iter)?;
        let mut sections = Vec::new();

        while let Some(section) = Self::parse_infobox_section(&mut iter)? {
            sections.push(section);
        }

        Ok(Self {
            title,
            sections
        })
    }

    fn parse_infobox_title(iter: &mut Peekable<Parser>) -> MdbookResult<String> {
        let mut title_heading_started = false;
        let mut title_contents: String = String::new();

        for event in iter {
            if let Event::Start(Tag::Heading(_, _, _)) = event {
                title_heading_started = true;
                continue;
            }
            
            if title_heading_started {
                if let Event::Text(text) = event {
                    title_contents += text.into_string().as_str();
                    
                    continue;
                } else if let Event::End(Tag::Heading(_, _, _)) = event {
                    return Ok(title_contents);
                }
            }
            
            return Err(anyhow!("unexpected event: {:?}", event));
        }

        return Err(anyhow!("failed to find infobox title"));
    }

    fn parse_infobox_section(iter: &mut Peekable<Parser>) -> MdbookResult<Option<InfoboxSection>> {
        while let Some(event) = iter.peek() {
            if let Event::Start(Tag::Heading(_, _, _)) = event {
                return Ok(Some(InfoboxSection::Field(Self::parse_infobox_field(iter)?)));
            } else if let Event::Start(Tag::Image(_, _, _)) = event {
                return Ok(Some(InfoboxSection::Image(Self::parse_infobox_image(iter)?)));
            }

            iter.next();
        }
        
        Ok(None)
    }

    fn parse_infobox_field(iter: &mut Peekable<Parser>) -> MdbookResult<InfoboxField> {
        // Parse name
        let mut name_heading_started = false;
        let mut name_contents = String::new();

        for event in &mut *iter {
            if let Event::Start(Tag::Heading(_, _, _)) = event {
                name_heading_started = true;
                continue;
            }
            
            if name_heading_started {
                if let Event::Text(text) = event {
                    name_contents += text.into_string().as_str();
                    
                    continue;
                } else if let Event::End(Tag::Heading(_, _, _)) = event {
                    break;
                }
            }
            
            return Err(anyhow!("unexpected event: {:?}", event));
        }

        let mut contents = String::new();

        // Parse contents
        while let Some(event) = iter.peek() {
            // Reached another heading, finish parsing the field
            if let Event::Start(Tag::Heading(_, _, _)) = event {
                break;
            }

            if let Event::Text(text) = event {
                contents += text.to_string().as_str();
            }

            iter.next();
        }

        Ok(InfoboxField {
            name: name_contents,
            contents,
        })
    }

    fn parse_infobox_image(iter: &mut Peekable<Parser>) -> MdbookResult<InfoboxImage> {
        match iter.next() {
            Some(Event::Start(Tag::Image(_, url, _))) => {
                // For some reason titles come with a text after the thing?
                let title = if let Some(Event::Text(title)) = iter.peek() {
                    Some(title.to_string())
                } else {
                    None
                };

                if title.is_some() {
                    iter.next();
                }

                assert!(std::matches!(iter.next(), Some(Event::End(Tag::Image(..)))));

                Ok(InfoboxImage { url: url.to_string(), title })
            },
            event => Err(anyhow!("unexpected event {:?}", event)),
        }
    }

    fn render_html(&self) -> String {
        let mut lines = vec![
            r##"<table class="infobox">"##.into(),
            "<thead>".into(),
            "<tr>".into(),
            format!(r##"<th colspan="2">{}</th>"##, self.title),
            "</tr>".into(),
            "</thead>".into(),
        ];

        for section in &self.sections {
            lines.push(section.render_html())
        }

        lines.push("</table>".into());

        lines.join("\n")
    }
}

impl InfoboxSection {
    pub fn render_html(&self) -> String {
        match &self {
            Self::Field(field) => Self::render_field_html(field),
            Self::Image(image) => Self::render_image_html(image),
        }
    }

    fn render_field_html(field: &InfoboxField) -> String {
        format!(r##"
<tr>
    <td>{}</td>
    <td>{}</td>
</tr>"##, field.name, field.contents)
    }

    fn render_image_html(image: &InfoboxImage) -> String {
        format!(r##"
<tr>
    <td colspan="2"><img src="{}" title="{}"/></td>
</tr>"##, image.url, image.title.clone().unwrap_or_default())
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_find_infoboxes_contents() {
        let document = r##"
# Sunshine
{{#infobox}}
# Infobox title
## Infobox field
Field contents
{{/infobox}}

# Description
"##;
        let expected_infobox_contents = r##"
# Infobox title
## Infobox field
Field contents
"##;

        let infoboxes_contents = find_infoboxes_contents(document);
        assert_eq!(1, infoboxes_contents.len());

        let (infobox_contents, _) = &infoboxes_contents[0];

        assert_eq!(expected_infobox_contents, infobox_contents);
    }

    #[test]
    fn test_from_markdown_contents_simple() {
        let infobox_contents = r##"
# Sunshine
## Name
Testing

## Birthday
1999-07-27

## Age
23 years
"##;

        let expected_infobox = Infobox {
            title: "Sunshine".into(),
            sections: vec![
                InfoboxSection::Field(InfoboxField { name: "Name".into(), contents: "Testing".into() }),
                InfoboxSection::Field(InfoboxField { name: "Birthday".into(), contents: "1999-07-27".into() }),
                InfoboxSection::Field(InfoboxField { name: "Age".into(), contents: "23 years".into() }),
            ],
        };

        assert_eq!(expected_infobox, Infobox::from_markdown_content(infobox_contents).unwrap());
    }

    #[test]
    fn test_from_markdown_contents_with_image() {
        let infobox_contents = r##"
# Sunshine
![image](images/test.jpg)

## Name
Testing
"##;

        let expected_infobox = Infobox {
            title: "Sunshine".into(),
            sections: vec![
                InfoboxSection::Image(InfoboxImage { title: Some("image".into()), url: "images/test.jpg".into() }),
                InfoboxSection::Field(InfoboxField { name: "Name".into(), contents: "Testing".into() }),
            ],
        };

        assert_eq!(expected_infobox, Infobox::from_markdown_content(infobox_contents).unwrap());
    }

    #[test]
    fn test_preprocessor() {
        let chapter_contents = r##"
# Sunshine

{{#infobox}}
# Sunshine
![image](images/test.jpg)

## Name
Testing
{{/infobox}}

# History
Teste
"##;

        let ctx = mock_context("html");
        let book = mock_book(chapter_contents);
        let expected_book = book.clone();

        assert_eq!(expected_book, InfoboxPreprocessor.run(&ctx, book).unwrap());
    }

    // taken from mdbook-admonish
    fn mock_context(renderer: &str) -> PreprocessorContext {
        let value = json!({
            "root": "/path/to/book",
            "config": {
                "book": {
                    "authors": ["AUTHOR"],
                    "language": "en",
                    "multilingual": false,
                    "src": "src",
                    "title": "TITLE"
                },
            },
            "renderer": renderer,
            "mdbook_version": "0.4.21"
        });

        serde_json::from_value(value).unwrap()
    }

    fn mock_book(content: &str) -> Book {
        serde_json::from_value(json!({
            "sections": [
                {
                    "Chapter": {
                        "name": "Chapter 1",
                        "content": content,
                        "number": [1],
                        "sub_items": [],
                        "path": "chapter_1.md",
                        "source_path": "chapter_1.md",
                        "parent_names": []
                    }
                }
            ],
            "__non_exhaustive": null
        }))
        .unwrap()
    }
}

