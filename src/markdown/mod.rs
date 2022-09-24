mod cache;
mod commonmark;
pub use commonmark::CommonMarkParser;

/// Generic for parsing markdown to html
pub trait MarkdownParser {
    /// Returns HTML parsed from the input `markdown`
    ///
    /// # Arguments
    ///
    /// * `markdown`: Input markdown (CommonMark)
    fn parse_to_html(&mut self, markdown: &str) -> String;
}
