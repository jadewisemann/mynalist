//! A small subset of the inline syntax (`packages/core/src/inline.ts`), enough to draw formatted rows in the
//! spike: `**bold**`, `*italic*` / `_italic_`, `` `code` ``, `[text](url)` and `#tag`. The full parser is the
//! next port.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    Plain(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { text: String, url: String },
    Tag(String),
}

fn find_from(s: &str, from: usize, pat: &str) -> Option<usize> {
    s.get(from..)?.find(pat).map(|i| i + from)
}

pub fn parse(src: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Plain(std::mem::take(plain)));
        }
    };
    while i < src.len() {
        let rest = &src[i..];
        let mut matched: Option<(Span, usize)> = None;
        if let Some(r) = rest.strip_prefix("**") {
            if let Some(end) = r.find("**").filter(|&e| e > 0) {
                matched = Some((Span::Bold(r[..end].to_owned()), 4 + end));
            }
        } else if rest.starts_with('`') {
            if let Some(end) = find_from(rest, 1, "`").filter(|&e| e > 1) {
                matched = Some((Span::Code(rest[1..end].to_owned()), end + 1));
            }
        } else if rest.starts_with('*') || rest.starts_with('_') {
            let mark = &rest[..1];
            if let Some(end) = find_from(rest, 1, mark).filter(|&e| e > 1) {
                matched = Some((Span::Italic(rest[1..end].to_owned()), end + 1));
            }
        } else if rest.starts_with('[') {
            if let Some(close) = rest.find("](") {
                if let Some(end) = find_from(rest, close + 2, ")") {
                    matched = Some((
                        Span::Link { text: rest[1..close].to_owned(), url: rest[close + 2..end].to_owned() },
                        end + 1,
                    ));
                }
            }
        } else if rest.starts_with('#') && (i == 0 || src[..i].ends_with(char::is_whitespace)) {
            let len = rest[1..].find(|c: char| c.is_whitespace() || c == ',' || c == '.').unwrap_or(rest.len() - 1);
            if len > 0 {
                matched = Some((Span::Tag(rest[..len + 1].to_owned()), len + 1));
            }
        }
        match matched {
            Some((span, len)) => {
                flush(&mut plain, &mut out);
                out.push(span);
                i += len;
            }
            None => {
                let c = rest.chars().next().expect("not at the end");
                plain.push(c);
                i += c.len_utf8();
            }
        }
    }
    flush(&mut plain, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::{parse, Span::*};

    #[test]
    fn parses_the_spike_subset() {
        assert_eq!(
            parse("a **b** *c* `d` [e](f) #g 한글"),
            vec![
                Plain("a ".into()),
                Bold("b".into()),
                Plain(" ".into()),
                Italic("c".into()),
                Plain(" ".into()),
                Code("d".into()),
                Plain(" ".into()),
                Link { text: "e".into(), url: "f".into() },
                Plain(" ".into()),
                Tag("#g".into()),
                Plain(" 한글".into()),
            ]
        );
        assert_eq!(parse("**"), vec![Plain("**".into())]);
        assert_eq!(parse("a#b"), vec![Plain("a#b".into())]);
    }
}
