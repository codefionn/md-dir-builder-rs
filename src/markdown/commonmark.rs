use super::MarkdownParser;

use pulldown_cmark::{html, Options, Parser};

pub struct CommonMarkParser {}

impl CommonMarkParser {
    fn create_options() -> Options {
        // Enable some default quality of live improvements
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TABLES);

        options
    }
}

impl Default for CommonMarkParser {
    fn default() -> Self {
        CommonMarkParser {}
    }
}

impl MarkdownParser for CommonMarkParser {
    fn parse_to_html(&mut self, markdown: &str) -> String {
        let parser = Parser::new_ext(markdown, Self::create_options());

        // Write to String buffer.
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        html_output
    }
}
