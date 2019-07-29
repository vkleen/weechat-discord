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
            weechat.color_codes("bold"),
            collect_styles(weechat, styles),
            weechat.color_codes("-bold")
        ),
        Italic(styles) => format!(
            "{}{}{}",
            weechat.color_codes("italic"),
            collect_styles(weechat, styles),
            weechat.color_codes("-italic")
        ),
        Underline(styles) => format!(
            "{}{}{}",
            weechat.color_codes("underline"),
            collect_styles(weechat, styles),
            weechat.color_codes("-underline")
        ),
        Strikethrough(styles) => format!(
            "{}~~{}~~{}",
            weechat.color_codes("red"),
            collect_styles(weechat, styles),
            weechat.color_codes("-red")
        ),
        Spoiler(styles) => format!(
            "{}||{}||{}",
            weechat.color_codes("italic"),
            collect_styles(weechat, styles),
            weechat.color_codes("-italic")
        ),
        Text(string) => string.to_owned(),
        InlineCode(string) | Code(_, string) => format!(
            "{}{}{}",
            weechat.color_codes("*8"),
            string,
            weechat.color_codes("reset")
        ),
    }
}
