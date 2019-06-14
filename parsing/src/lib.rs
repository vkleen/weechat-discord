pub use simple_ast::MarkdownNode;
use simple_ast::{Parser, Rule, Styled};

pub fn parse_markdown(str: &str) -> Styled<MarkdownNode> {
    use simple_ast::markdown_rules::*;
    let rules: &[&dyn Rule<MarkdownNode>] = &[
        &Escape,
        &Newline,
        &Bold,
        &Underline,
        &Italic,
        &Strikethrough,
        &Spoiler,
        &Code,
        &InlineCode,
        &Text,
    ];

    Parser::with_rules(rules).parse(str)
}

pub fn weechat_arg_strip(str: &str) -> String {
    str.trim().replace(' ', "_")
}
