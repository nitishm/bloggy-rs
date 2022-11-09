use pulldown_cmark::{html, Options as MDOptions, Parser};

pub fn render_markdown<'input>(input: &'input str, options: MDOptions) -> String {
    let parser = Parser::new_ext(input, options);
    let mut html_output: String = String::with_capacity(input.len() * 3 / 2);
    html::push_html(&mut html_output, parser);
    return html_output;
}

#[cfg(test)]
mod tests {
    use super::render_markdown;
    use pulldown_cmark::Options as MDOptions;

    #[test]
    fn render_test() {
        let input = "# This is a title line";
        let output = render_markdown(input, MDOptions::empty());
        let expected_str = "<h1>This is a title line</h1>\n";
        assert_eq!(output, expected_str)
    }
}
