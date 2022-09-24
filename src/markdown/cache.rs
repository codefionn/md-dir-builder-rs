use ahash::{AHasher, RandomState};

use super::MarkdownParser;

pub struct CacheMarkdown {
    markdown_parser: Box<dyn MarkdownParser>,
    cache_map: std::collections::HashMap<String, String, RandomState>,
}

impl MarkdownParser for CacheMarkdown {
    fn parse_to_html(&mut self, markdown: &str) -> String {
        let result = self.markdown_parser.parse_to_html(markdown);
        self.cache_map.insert(markdown.to_string(), result.clone());

        result
    }
}
