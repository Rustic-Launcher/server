//! A small MiniMessage-like tag syntax (written for Rustic; same spirit, own implementation).
//!
//! Grammar, informally:
//!
//! ```text
//! input  := (text | escape | tag)*
//! escape := '\' ('<' | '\')
//! tag    := '<' ['/' | '!'] name (':' arg)* '>'
//! arg    := bare | "'" quoted "'" | '"' quoted '"'      (quotes allow ':' and '>'; \' escapes)
//! ```
//!
//! Opening tags push a style onto a stack; `</name>` removes the most recent matching one and
//! `</>` the most recent of any kind; `<reset>` clears the stack. Unknown tags stay literal.

use super::{ClickEvent, Color, Decoration, HoverEvent, Style, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Color,
    Decoration,
    Other,
}

#[derive(Debug)]
struct Frame {
    key: String,
    kind: Kind,
    style: Style,
}

struct Parser {
    parts: Vec<Text>,
    stack: Vec<Frame>,
    buf: String,
    saw_tag: bool,
}

pub(super) fn parse(input: &str) -> Text {
    let chars: Vec<char> = input.chars().collect();
    let mut p = Parser {
        parts: Vec::new(),
        stack: Vec::new(),
        buf: String::new(),
        saw_tag: false,
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && matches!(chars.get(i + 1), Some('<') | Some('\\')) {
            p.buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '<'
            && let Some((content, end)) = read_tag(&chars, i)
            && p.apply(&content)
        {
            p.saw_tag = true;
            i = end;
            continue;
        }
        p.buf.push(c);
        i += 1;
    }
    p.flush();
    if !p.saw_tag {
        return Text::new(p.parts.first().and_then(Text::literal_text).unwrap_or(""));
    }
    Text::empty().extend(p.parts)
}

/// Reads `<...>` starting at `chars[start] == '<'`: the content between the brackets and the
/// index after `>`. Quotes inside may contain `>`.
fn read_tag(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start + 1;
    let mut quote: Option<char> = None;
    let mut content = String::new();
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == '\\' && i + 1 < chars.len() {
                    content.push(c);
                    content.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '>' => {
                    return (!content.is_empty()).then_some((content, i + 1));
                }
                // Tags have no whitespace outside quotes: `a < b > c` is text.
                '<' => return None,
                c if c.is_whitespace() => return None,
                '\'' | '"' => quote = Some(c),
                _ => {}
            },
        }
        content.push(c);
        i += 1;
    }
    None
}

/// Splits tag content on `:` outside quotes, unquoting arguments.
fn split_args(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    if let Some(&n) = chars.peek()
                        && (n == q || n == '\\')
                    {
                        cur.push(n);
                        chars.next();
                        continue;
                    }
                    cur.push(c);
                } else if c == q {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                ':' => out.push(std::mem::take(&mut cur)),
                '\'' | '"' => quote = Some(c),
                _ => cur.push(c),
            },
        }
    }
    out.push(cur);
    out
}

fn decoration(name: &str) -> Option<Decoration> {
    Some(match name {
        "bold" | "b" => Decoration::Bold,
        "italic" | "i" | "em" => Decoration::Italic,
        "underlined" | "underline" | "u" => Decoration::Underlined,
        "strikethrough" | "st" => Decoration::Strikethrough,
        "obfuscated" | "obf" => Decoration::Obfuscated,
        _ => return None,
    })
}

fn is_color_tag(name: &str) -> bool {
    matches!(name, "color" | "colour" | "c")
}

impl Parser {
    fn current_style(&self) -> Style {
        let mut s = Style::default();
        for f in &self.stack {
            s.apply(&f.style);
        }
        s
    }

    fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.buf);
        let style = self.current_style();
        if let Some(last) = self.parts.last_mut()
            && *last.style() == style
            && last.children().is_empty()
            && last.literal_text().is_some()
        {
            last.push_literal(&text);
            return;
        }
        self.parts.push(Text::new(text).with_style(style));
    }

    fn push_frame(&mut self, key: &str, kind: Kind, style: Style) {
        self.flush();
        self.stack.push(Frame {
            key: key.to_owned(),
            kind,
            style,
        });
    }

    fn push_component(&mut self, t: Text) {
        self.flush();
        let style = self.current_style();
        let mut merged = style;
        merged.apply(t.style());
        self.parts.push(t.with_style(merged));
    }

    /// Applies one tag; false means "not a tag we know": keep it as literal text.
    fn apply(&mut self, content: &str) -> bool {
        if let Some(rest) = content.strip_prefix('/') {
            let name = split_args(rest)
                .into_iter()
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            self.close(name.trim());
            return true;
        }
        if let Some(rest) = content.strip_prefix('!') {
            let name = rest.trim().to_ascii_lowercase();
            return match decoration(&name) {
                Some(d) => {
                    let mut s = Style::default();
                    s.set(d, false);
                    self.push_frame(d.name(), Kind::Decoration, s);
                    true
                }
                None => false,
            };
        }

        let args = split_args(content);
        let name = args[0].trim().to_ascii_lowercase();
        let arg = |n: usize| args.get(n).map(String::as_str);

        if let Some(color) = Color::parse(&name) {
            self.push_frame(&name, Kind::Color, color_style(color));
            return true;
        }
        if is_color_tag(&name) {
            return match arg(1).and_then(Color::parse) {
                Some(color) => {
                    self.push_frame(&name, Kind::Color, color_style(color));
                    true
                }
                None => false,
            };
        }
        if let Some(d) = decoration(&name) {
            let on = !matches!(arg(1), Some("false"));
            let mut s = Style::default();
            s.set(d, on);
            self.push_frame(d.name(), Kind::Decoration, s);
            return true;
        }
        match name.as_str() {
            "reset" => {
                self.flush();
                self.stack.clear();
                true
            }
            "newline" | "br" => {
                self.buf.push('\n');
                true
            }
            "hover" => match (arg(1), arg(2)) {
                (Some("show_text"), Some(value)) => {
                    let s = Style {
                        hover: Some(HoverEvent::ShowText(Box::new(parse(value)))),
                        ..Style::default()
                    };
                    self.push_frame("hover", Kind::Other, s);
                    true
                }
                _ => false,
            },
            "click" => {
                let Some(value) = arg(2) else { return false };
                let click = match arg(1) {
                    Some("run_command") => ClickEvent::RunCommand(value.to_owned()),
                    Some("suggest_command") => ClickEvent::SuggestCommand(value.to_owned()),
                    Some("open_url") => ClickEvent::OpenUrl(value.to_owned()),
                    Some("copy_to_clipboard") => ClickEvent::CopyToClipboard(value.to_owned()),
                    Some("change_page") => match value.parse() {
                        Ok(p) => ClickEvent::ChangePage(p),
                        Err(_) => return false,
                    },
                    _ => return false,
                };
                let s = Style {
                    click: Some(click),
                    ..Style::default()
                };
                self.push_frame("click", Kind::Other, s);
                true
            }
            "insert" | "insertion" => match arg(1) {
                Some(v) => {
                    let s = Style {
                        insertion: Some(v.to_owned()),
                        ..Style::default()
                    };
                    self.push_frame("insert", Kind::Other, s);
                    true
                }
                None => false,
            },
            "font" => match arg(1) {
                Some(v) => {
                    let s = Style {
                        font: Some(v.to_owned()),
                        ..Style::default()
                    };
                    self.push_frame("font", Kind::Other, s);
                    true
                }
                None => false,
            },
            "lang" | "tr" | "translate" => match arg(1) {
                Some(key) => {
                    let mut t = Text::translatable(key);
                    for a in args.iter().skip(2) {
                        t = t.arg(parse(a));
                    }
                    self.push_component(t);
                    true
                }
                None => false,
            },
            "key" => match arg(1) {
                Some(key) => {
                    self.push_component(Text::keybind(key));
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn close(&mut self, name: &str) {
        self.flush();
        if name.is_empty() {
            self.stack.pop();
            return;
        }
        let key = decoration(name)
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|| match name {
                "insertion" => "insert".to_owned(),
                other => other.to_owned(),
            });
        if let Some(i) = self.stack.iter().rposition(|f| f.key == key) {
            self.stack.remove(i);
            return;
        }
        // `</color>` closes the latest color, and so does a mismatched color name.
        if (is_color_tag(name) || Color::parse(name).is_some())
            && let Some(i) = self.stack.iter().rposition(|f| f.kind == Kind::Color)
        {
            self.stack.remove(i);
        }
    }
}

fn color_style(c: Color) -> Style {
    Style {
        color: Some(c),
        ..Style::default()
    }
}

/// Escapes `input` so [`Text::mini`] shows it literally (use it for player-provided text you
/// splice into a template).
pub fn escape_mini(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == '<' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::NamedColor;
    use super::*;

    fn json(t: &Text) -> serde_json::Value {
        serde_json::from_str(&t.to_json()).unwrap()
    }

    #[test]
    fn plain_input() {
        assert_eq!(Text::mini("hello"), Text::new("hello"));
        assert_eq!(Text::mini("a < b > c"), Text::new("a < b > c"));
        assert_eq!(Text::mini(""), Text::new(""));
    }

    #[test]
    fn colors_nesting_and_closing() {
        let t = Text::mini("<red>Hello <bold>big</bold> world</red>!");
        let v = json(&t);
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e.len(), 4);
        assert_eq!(
            (e[0]["text"].as_str(), e[0]["color"].as_str()),
            (Some("Hello "), Some("red"))
        );
        assert_eq!(e[1]["text"], "big");
        assert_eq!(e[1]["color"], "red");
        assert_eq!(e[1]["bold"], true);
        assert_eq!(e[2]["text"], " world");
        assert!(e[2].get("bold").is_none());
        assert_eq!(e[3]["text"], "!");
        assert!(e[3].get("color").is_none());
        assert_eq!(t.plain(), "Hello big world!");
    }

    #[test]
    fn hex_and_color_tag_and_grey() {
        let v = json(&Text::mini(
            "<#ff8800>a<color:dark_grey>b</color>c<c:#00f>d",
        ));
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e[0]["color"], "#FF8800");
        assert_eq!(e[1]["color"], "dark_gray");
        assert_eq!(e[2]["color"], "#FF8800");
        assert_eq!(e[3]["color"], "#0000FF");
    }

    #[test]
    fn aliases_negation_and_reset() {
        let v = json(&Text::mini("<b><i>x</i><!italic>y<reset>z"));
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e[0]["bold"], true);
        assert_eq!(e[0]["italic"], true);
        assert_eq!(e[1]["bold"], true);
        assert_eq!(e[1]["italic"], false);
        assert_eq!(e[2]["text"], "z");
        assert!(e[2].get("bold").is_none());
        let v = json(&Text::mini("<bold:false>n"));
        assert_eq!(v["extra"][0]["bold"], false);
    }

    #[test]
    fn hover_and_click() {
        let t = Text::mini(
            "<hover:show_text:'<green>Go to <b>spawn</b>'><click:run_command:'/spawn'>[Spawn]</click></hover> after",
        );
        let v = json(&t);
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e[0]["text"], "[Spawn]");
        assert_eq!(e[0]["click_event"]["action"], "run_command");
        assert_eq!(e[0]["click_event"]["command"], "/spawn");
        let hover = &e[0]["hover_event"]["value"];
        assert_eq!(hover["extra"][0]["text"], "Go to ");
        assert_eq!(hover["extra"][0]["color"], "green");
        assert_eq!(hover["extra"][1]["bold"], true);
        assert_eq!(e[1]["text"], " after");
        assert!(e[1].get("click_event").is_none());
        assert!(e[1].get("hover_event").is_none());
    }

    #[test]
    fn click_variants_and_quotes() {
        let t = Text::mini(
            r#"<click:open_url:"https://a.b/c?d=1:2">u</click><click:suggest_command:'/msg it\'s '>s<click:copy_to_clipboard:v>c"#,
        );
        let v = json(&t);
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e[0]["click_event"]["url"], "https://a.b/c?d=1:2");
        assert_eq!(e[1]["click_event"]["command"], "/msg it's ");
        assert_eq!(e[2]["click_event"]["value"], "v");
        // An unknown click action stays literal.
        assert_eq!(
            Text::mini("<click:explode:x>y").plain(),
            "<click:explode:x>y"
        );
    }

    #[test]
    fn unknown_and_malformed_tags_are_literal() {
        assert_eq!(Text::mini("<notatag>x").plain(), "<notatag>x");
        assert_eq!(Text::mini("<red x").plain(), "<red x");
        assert_eq!(Text::mini("<>").plain(), "<>");
        assert_eq!(Text::mini("<color:nope>x").plain(), "<color:nope>x");
        let t = Text::mini("<red>a <b c</red>");
        assert_eq!(t.plain(), "a <b c");
    }

    #[test]
    fn escapes() {
        let t = Text::mini(r"\<red>not red\\<green>green");
        assert_eq!(t.plain(), r"<red>not red\green");
        let v = json(&t);
        let e = v["extra"].as_array().unwrap();
        assert!(e[0].get("color").is_none());
        assert_eq!(e[1]["color"], "green");
        let name = "<bold>Evil\\";
        let escaped = escape_mini(name);
        assert_eq!(Text::mini(&format!("<gray>{escaped}")).plain(), name);
    }

    #[test]
    fn close_all_and_unmatched_close() {
        let v = json(&Text::mini("<red><bold>a</>b</>c</blue>d"));
        let e = v["extra"].as_array().unwrap();
        assert_eq!(e[0]["bold"], true);
        assert_eq!(e[1]["text"], "b");
        assert_eq!(e[1]["color"], "red");
        assert!(e[1].get("bold").is_none());
        assert_eq!(e[2]["text"], "cd");
    }

    #[test]
    fn mismatched_color_close_closes_latest_color() {
        let v = json(&Text::mini("<red>a</blue>b"));
        assert_eq!(v["extra"][1]["text"], "b");
        assert!(v["extra"][1].get("color").is_none());
    }

    #[test]
    fn newline_lang_key_insert_font() {
        let t = Text::mini(
            "a<br>b<newline><yellow><lang:block.minecraft.stone:'<red>x'> <key:key.jump><insert:hi>i</insert><font:uniform>f",
        );
        let v = json(&t);
        assert!(t.plain().starts_with("a\nb\n"));
        let e = v["extra"].as_array().unwrap();
        let tr = e.iter().find(|x| x.get("translate").is_some()).unwrap();
        assert_eq!(tr["translate"], "block.minecraft.stone");
        assert_eq!(tr["color"], "yellow");
        assert_eq!(tr["with"][0]["extra"][0]["color"], "red");
        let kb = e.iter().find(|x| x.get("keybind").is_some()).unwrap();
        assert_eq!(kb["keybind"], "key.jump");
        let ins = e.iter().find(|x| x["text"] == "i").unwrap();
        assert_eq!(ins["insertion"], "hi");
        let f = e.iter().find(|x| x["text"] == "f").unwrap();
        assert_eq!(f["font"], "uniform");
        assert!(f.get("insertion").is_none());
    }

    #[test]
    fn styles_inherit_correctly_into_translations() {
        let t = Text::mini("<gold><lang:x>");
        let v = json(&t);
        assert_eq!(v["extra"][0]["color"], NamedColor::Gold.name());
    }
}
