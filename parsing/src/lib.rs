use pest::Parser;

#[derive(pest_derive::Parser)]
#[grammar = "discord_grammar.pest"]
pub struct MarkdownParser;

#[derive(Debug, PartialEq, Clone)]
pub struct Styled(pub Vec<Style>);

impl Styled {
    pub fn as_markdown(&self) -> String {
        self.0
            .iter()
            .map(Style::as_markdown)
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Style {
    Bold(Vec<Style>),
    Italic(Vec<Style>),
    Underline(Vec<Style>),
    Strikethrough(Vec<Style>),
    Plain(String),
}

impl Style {
    // TODO: take into account _/* italic syntax
    pub fn as_markdown(&self) -> String {
        use Style::*;
        match self {
            Bold(style) => format!(
                "**{}**",
                style
                    .iter()
                    .map(Style::as_markdown)
                    .collect::<Vec<_>>()
                    .join("")
            ),
            Italic(style) => format!(
                "_{}_",
                style
                    .iter()
                    .map(Style::as_markdown)
                    .collect::<Vec<_>>()
                    .join("")
            ),
            Underline(style) => format!(
                "__{}__",
                style
                    .iter()
                    .map(Style::as_markdown)
                    .collect::<Vec<_>>()
                    .join("")
            ),
            Strikethrough(style) => format!(
                "~~{}~~",
                style
                    .iter()
                    .map(Style::as_markdown)
                    .collect::<Vec<_>>()
                    .join("")
            ),
            Plain(string) => string.to_owned(),
        }
    }
}

fn lower(pair: pest::iterators::Pair<Rule>) -> Vec<Style> {
    let rule = pair.as_rule();
    match rule {
        Rule::all => {
            let mut result = Vec::new();
            for pair in pair.into_inner() {
                result.extend(lower(pair))
            }
            result
        }
        Rule::italic => {
            let mut result = Vec::new();
            for pair in pair.into_inner() {
                result.extend(lower(pair))
            }
            vec![Style::Italic(result)]
        }
        Rule::underline_inner | Rule::strike_inner | Rule::bold_inner | Rule::italic_inner => {
            let content = pair.as_str();
            match pair.into_inner().next() {
                Some(inner) => lower(inner),
                None => vec![Style::Plain(content.to_owned())],
            }
        }
        Rule::bold => {
            let mut result = Vec::new();
            for pair in pair.into_inner() {
                result.extend(lower(pair))
            }
            vec![Style::Bold(result)]
        }
        Rule::underline => {
            let mut result = Vec::new();
            for pair in pair.into_inner() {
                result.extend(lower(pair))
            }
            vec![Style::Underline(result)]
        }
        Rule::strike => {
            let mut result = Vec::new();
            for pair in pair.into_inner() {
                result.extend(lower(pair))
            }
            vec![Style::Strikethrough(result)]
        }
        Rule::plain | Rule::WS => vec![Style::Plain(pair.as_str().to_owned())],

        _ => unimplemented!(),
    }
}

pub fn parse_markdown(str: &str) -> Styled {
    // Should be infallible
    let mut rules = MarkdownParser::parse(Rule::all, str).unwrap();

    Styled(lower(rules.next().unwrap()))
}

pub fn weechat_arg_strip(str: &str) -> String {
    str.trim().replace(' ', "_")
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {
        use super::{Style::*, Styled};
        let target = Styled(vec![Bold(vec![
            Italic(vec![Plain("Hi".to_string())]),
            Plain(" ".to_string()),
            Underline(vec![Plain("there".to_string())]),
        ])]);

        assert_eq!(target, super::parse_markdown("**_Hi_ __there__**"));
    }
}
