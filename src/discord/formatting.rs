use parsing::{self, MarkdownNode};
use std::rc::Rc;
use std::sync::RwLock;
use weechat::Weechat;

pub fn discord_to_weechat(weechat: &Weechat, msg: &str) -> String {
    let ast = parsing::parse_markdown(msg);

    let mut out = String::new();
    for node in &ast.0 {
        out.push_str(&discord_to_weechat_reducer(
            &weechat,
            &*node.read().unwrap(),
        ))
    }
    out
}

fn collect_styles(weechat: &Weechat, styles: &[Rc<RwLock<MarkdownNode>>]) -> String {
    styles
        .iter()
        .map(|s| discord_to_weechat_reducer(&weechat, &*s.read().unwrap()))
        .collect::<Vec<_>>()
        .join("")
}

// TODO: Spoilers, code syntax highlighting?
fn discord_to_weechat_reducer(weechat: &Weechat, node: &MarkdownNode) -> String {
    use MarkdownNode::*;
    match node {
        Bold(styles) => format!(
            "{}{}{}",
            weechat.color("bold"),
            collect_styles(weechat, styles),
            weechat.color("-bold")
        ),
        Italic(styles) => format!(
            "{}{}{}",
            weechat.color("italic"),
            collect_styles(weechat, styles),
            weechat.color("-italic")
        ),
        Underline(styles) => format!(
            "{}{}{}",
            weechat.color("underline"),
            collect_styles(weechat, styles),
            weechat.color("-underline")
        ),
        Strikethrough(styles) => format!(
            "{}~~{}~~{}",
            weechat.color("red"),
            collect_styles(weechat, styles),
            weechat.color("-red")
        ),
        Spoiler(styles) => format!(
            "{}||{}||{}",
            weechat.color("italic"),
            collect_styles(weechat, styles),
            weechat.color("-italic")
        ),
        Text(string) => string.to_owned(),
        InlineCode(string) => format!(
            "{}{}{}",
            weechat.color("*8"),
            string,
            weechat.color("reset")
        ),
        Code(language, text) => {
            let (fmt, reset) = (weechat.color("*8"), weechat.color("reset"));

            format!(
                "```{}\n{}\n```",
                language,
                text.lines()
                    .map(|l| format!("{}{}{}", fmt, l, reset))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
        BlockQuote(styles) => collect_styles(weechat, styles)
            .lines()
            .fold(String::new(), |acc, x| format!("{}\n▎{}", acc, x)),
        SingleBlockQuote(styles) => collect_styles(weechat, styles)
            .lines()
            .fold(String::new(), |acc, x| {
                format!("{}▎{}\n", acc, strip_leading_bracket(x))
            }),
    }
}

fn strip_leading_bracket(line: &str) -> &str {
    &line[line.find("> ").map(|x| x + 2).unwrap_or(0)..]
}
