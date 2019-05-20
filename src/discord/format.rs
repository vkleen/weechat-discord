use crate::ffi::color_codes;
use parsing::{self, MarkdownNode};
use std::rc::Rc;
use std::sync::RwLock;

pub fn discord_to_weechat(msg: &str) -> String {
    let ast = parsing::parse_markdown(msg);

    let mut out = String::new();
    for node in &ast.0 {
        eprintln!("{:#?}", node.read().unwrap());
        out.push_str(&discord_to_weechat_reducer(&*node.read().unwrap()))
    }
    out
}

fn collect_styles(styles: &[Rc<RwLock<MarkdownNode>>]) -> String {
    styles
        .iter()
        .map(|s| discord_to_weechat_reducer(&*s.read().unwrap()))
        .collect::<Vec<_>>()
        .join("")
}

// TODO: Spoilers, code syntax highlighting?
fn discord_to_weechat_reducer(node: &MarkdownNode) -> String {
    use MarkdownNode::*;
    match node {
        Bold(styles) => format!(
            "{}{}{}",
            color_codes("bold"),
            collect_styles(styles),
            color_codes("-bold")
        ),
        Italic(styles) => format!(
            "{}{}{}",
            color_codes("italic"),
            collect_styles(styles),
            color_codes("-italic")
        ),
        Underline(styles) => format!(
            "{}{}{}",
            color_codes("underline"),
            collect_styles(styles),
            color_codes("-underline")
        ),
        Strikethrough(styles) => format!(
            "{}~~{}~~{}",
            color_codes("red"),
            collect_styles(styles),
            color_codes("-red")
        ),
        Spoiler(styles) => format!(
            "{}||{}||{}",
            color_codes("italic"),
            collect_styles(styles),
            color_codes("-italic")
        ),
        Text(string) => string.to_owned(),
        InlineCode(string) | Code(_, string) => {
            format!("{}{}{}", color_codes("*8"), string, color_codes("reset"))
        }
    }
}
