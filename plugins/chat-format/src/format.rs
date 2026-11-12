//! Pure text processing for chat lines: legacy `&` codes to MiniMessage-like tags, a sanitizer
//! that keeps only colour/decoration tags in player text, `@mention` detection and template
//! placeholders. Everything here works on strings and is unit-tested on the host.

use rustic::Color;
use rustic::text::escape_mini;

/// Decoration tags a player with `chatformat.color` may use (and their short forms).
const DECORATIONS: &[&str] = &[
    "bold",
    "b",
    "italic",
    "i",
    "em",
    "underlined",
    "u",
    "strikethrough",
    "st",
    "obfuscated",
    "obf",
];

/// Replaces `{key}` placeholders. Values are inserted as given (escape them first when they
/// come from players).
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.iter().find(|(k, _)| *k == key) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Converts `&`/`§` legacy codes (`&a`, `&l`, `&r`, `&#RRGGBB`, `&x&R&R&G&G&B&B`) to tags.
/// Every tag it opens is closed at the end, so the converted text never leaks style into what
/// follows it. A colour code ends the decorations opened before it, as in vanilla; `&r` closes
/// everything this text opened (never the surrounding template's styles).
pub fn legacy_to_mini(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 16);
    let mut open: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if (c == '&' || c == '§') && i + 1 < chars.len() {
            let code = chars[i + 1].to_ascii_lowercase();
            // &#RRGGBB
            if code == '#' && i + 8 <= chars.len() {
                let hex: String = chars[i + 2..i + 8].iter().collect();
                if hex.chars().all(|h| h.is_ascii_hexdigit()) {
                    close_decorations(&mut out, &mut open);
                    let tag = format!("#{}", hex.to_ascii_uppercase());
                    push_tag(&mut out, &mut open, tag);
                    i += 8;
                    continue;
                }
            }
            // &x&R&R&G&G&B&B
            if code == 'x' && i + 14 <= chars.len() {
                let mut hex = String::new();
                let mut ok = true;
                for k in 0..6 {
                    let marker = chars[i + 2 + k * 2];
                    let digit = chars[i + 3 + k * 2];
                    if (marker == '&' || marker == '§') && digit.is_ascii_hexdigit() {
                        hex.push(digit.to_ascii_uppercase());
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    close_decorations(&mut out, &mut open);
                    push_tag(&mut out, &mut open, format!("#{hex}"));
                    i += 14;
                    continue;
                }
            }
            if let Some(color) = rustic::text::NamedColor::from_code(code) {
                close_decorations(&mut out, &mut open);
                push_tag(&mut out, &mut open, color.name().to_owned());
                i += 2;
                continue;
            }
            let deco = match code {
                'k' => Some("obfuscated"),
                'l' => Some("bold"),
                'm' => Some("strikethrough"),
                'n' => Some("underlined"),
                'o' => Some("italic"),
                _ => None,
            };
            if let Some(d) = deco {
                push_tag(&mut out, &mut open, d.to_owned());
                i += 2;
                continue;
            }
            if code == 'r' {
                close_all(&mut out, &mut open);
                i += 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    close_all(&mut out, &mut open);
    out
}

fn push_tag(out: &mut String, open: &mut Vec<String>, tag: String) {
    out.push('<');
    out.push_str(&tag);
    out.push('>');
    open.push(tag);
}

fn is_decoration(tag: &str) -> bool {
    DECORATIONS.contains(&tag)
}

fn close_decorations(out: &mut String, open: &mut Vec<String>) {
    while let Some(pos) = open.iter().rposition(|t| is_decoration(t)) {
        let tag = open.remove(pos);
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
    }
}

fn close_all(out: &mut String, open: &mut Vec<String>) {
    while let Some(tag) = open.pop() {
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
    }
}

/// Whether a tag (the text between `<` and `>`) is a colour or decoration tag a player may use:
/// `red`, `#ff8800`, `color:red`, `bold`, `!italic`, and the closing forms `</red>`, `</bold>`.
/// `</>`, `<reset>`, hover/click/insert/font and anything else is refused.
pub fn is_style_tag(tag: &str) -> bool {
    let tag = tag.trim();
    if let Some(negated) = tag.strip_prefix('!') {
        return is_decoration(&negated.trim().to_ascii_lowercase());
    }
    let body = tag.strip_prefix('/').unwrap_or(tag).to_ascii_lowercase();
    if body.is_empty() {
        return false;
    }
    if is_decoration(&body) || Color::parse(&body).is_some() {
        return true;
    }
    if let Some(rest) = body
        .strip_prefix("color:")
        .or_else(|| body.strip_prefix("colour:"))
        .or_else(|| body.strip_prefix("c:"))
    {
        return Color::parse(rest).is_some();
    }
    false
}

/// Player text for a player allowed to colour it: legacy codes become tags, colour and
/// decoration tags are kept, every other `<` and every `\` is escaped.
pub fn sanitize_colored(input: &str) -> String {
    let converted = legacy_to_mini(input);
    let mut out = String::with_capacity(converted.len() + 8);
    let mut rest = converted.as_str();
    while let Some(c) = rest.chars().next() {
        match c {
            '\\' => {
                out.push_str("\\\\");
                rest = &rest[1..];
            }
            '<' => {
                let after = &rest[1..];
                let end = after.find('>');
                let next_open = after.find('<');
                match end {
                    Some(e) if next_open.is_none_or(|o| o > e) && is_style_tag(&after[..e]) => {
                        out.push('<');
                        out.push_str(&after[..e]);
                        out.push('>');
                        rest = &after[e + 1..];
                    }
                    _ => {
                        out.push_str("\\<");
                        rest = after;
                    }
                }
            }
            _ => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Player text for a player who may not colour it: legacy codes are shown literally and tags
/// are escaped.
pub fn sanitize_plain(input: &str) -> String {
    escape_mini(input)
}

/// A mention found in a message: the byte range of `@name` in the processed text and the name
/// as typed (without `@`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub start: usize,
    pub end: usize,
    pub name: String,
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Finds `@name` tokens (1 to 16 name characters, not preceded by a name character) outside of
/// tags and escapes in MiniMessage-like text.
pub fn find_mentions(mini: &str) -> Vec<Mention> {
    let bytes = mini.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'<' => match mini[i + 1..].find('>') {
                Some(e) => i += e + 2,
                None => i += 1,
            },
            b'@' => {
                let preceded = i > 0 && is_name_char(bytes[i - 1] as char);
                let start = i;
                let mut j = i + 1;
                while j < bytes.len() && is_name_char(bytes[j] as char) && j - i <= 16 {
                    j += 1;
                }
                let len = j - i - 1;
                let followed = j < bytes.len() && is_name_char(bytes[j] as char);
                if !preceded && (1..=16).contains(&len) && !followed {
                    found.push(Mention {
                        start,
                        end: j,
                        name: mini[i + 1..j].to_owned(),
                    });
                }
                i = j.max(i + 1);
            }
            _ => i += 1,
        }
    }
    found
}

/// Replaces the mentions `resolve` accepts with `format` (`{name}` = the resolved name).
/// Returns the new text and the resolved names, in order, without duplicates.
pub fn apply_mentions(
    mini: &str,
    format: &str,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> (String, Vec<String>) {
    let mut out = String::with_capacity(mini.len() + 16);
    let mut names: Vec<String> = Vec::new();
    let mut last = 0;
    for m in find_mentions(mini) {
        let Some(name) = resolve(&m.name) else {
            continue;
        };
        out.push_str(&mini[last..m.start]);
        out.push_str(&fill(format, &[("name", &escape_mini(&name))]));
        last = m.end;
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    out.push_str(&mini[last..]);
    (out, names)
}

/// Makes a value safe inside a quoted tag argument (`<hover:show_text:'...'>`).
pub fn quote_safe(value: &str) -> String {
    value.replace(['\'', '"'], "")
}

/// The clickable, hoverable name: `hover` and `click` are filled templates.
pub fn interactive_name(name: &str, hover: &str, click: &str) -> String {
    let mut out = String::new();
    if !hover.is_empty() {
        out.push_str("<hover:show_text:'");
        out.push_str(&quote_safe_tagged(hover));
        out.push_str("'>");
    }
    if !click.is_empty() {
        out.push_str("<click:suggest_command:'");
        out.push_str(&quote_safe(click));
        out.push_str("'>");
    }
    out.push_str(&escape_mini(name));
    if !click.is_empty() {
        out.push_str("</click>");
    }
    if !hover.is_empty() {
        out.push_str("</hover>");
    }
    out
}

/// Like [`quote_safe`] for mini text that is itself parsed (keeps tags, drops quotes).
fn quote_safe_tagged(value: &str) -> String {
    value.replace('\'', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustic::Text;

    #[test]
    fn fill_replaces_known_and_keeps_unknown() {
        assert_eq!(
            fill("{a}-{b}-{c}", &[("a", "1"), ("b", "2")]),
            "1-2-{c}".to_owned()
        );
        assert_eq!(fill("no { end", &[]), "no { end");
    }

    #[test]
    fn legacy_conversion() {
        assert_eq!(legacy_to_mini("&aHi"), "<green>Hi</green>");
        assert_eq!(
            legacy_to_mini("&l&cX&ly"),
            "<bold></bold><red>X<bold>y</bold></red>"
        );
        assert_eq!(legacy_to_mini("&#ff8800o"), "<#FF8800>o</#FF8800>");
        assert_eq!(legacy_to_mini("&x&f&f&0&0&0&0r"), "<#FF0000>r</#FF0000>");
        assert_eq!(legacy_to_mini("&ca&rb"), "<red>a</red>b");
        assert_eq!(legacy_to_mini("&zno & code"), "&zno & code");
        // Renders: the colour ends with the converted text.
        let t = Text::mini(&format!("{}after", legacy_to_mini("&cred")));
        assert_eq!(t.plain(), "redafter");
    }

    #[test]
    fn style_tags() {
        for ok in [
            "red",
            "/red",
            "#abc",
            "#AABBCC",
            "bold",
            "!italic",
            "/b",
            "color:red",
            "dark_grey",
        ] {
            assert!(is_style_tag(ok), "{ok}");
        }
        for bad in [
            "",
            "/",
            "reset",
            "click:run_command:'/op me'",
            "hover:show_text:'x'",
            "insert:x",
            "font:x",
            "lang:x",
            "key:key.jump",
            "!red",
            "newline",
        ] {
            assert!(!is_style_tag(bad), "{bad}");
        }
    }

    #[test]
    fn sanitizer_keeps_styles_and_escapes_the_rest() {
        assert_eq!(sanitize_colored("<red>hi</red>"), "<red>hi</red>");
        assert_eq!(
            sanitize_colored("<click:run_command:'/op me'>x"),
            "\\<click:run_command:'/op me'>x"
        );
        assert_eq!(sanitize_colored("a < b"), "a \\< b");
        assert_eq!(sanitize_colored("a\\b"), "a\\\\b");
        assert_eq!(sanitize_colored("&aok"), "<green>ok</green>");
        assert_eq!(sanitize_colored("<reset>x"), "\\<reset>x");
        assert_eq!(sanitize_colored("<</red>"), "\\<</red>");
        let shown = Text::mini(&sanitize_colored("<click:run_command:'/op me'>x")).plain();
        assert_eq!(shown, "<click:run_command:'/op me'>x");
        assert_eq!(Text::mini(&sanitize_plain("&a<red>x")).plain(), "&a<red>x");
    }

    #[test]
    fn mentions_are_found_outside_tags() {
        let m = find_mentions("hi @Steve and @alex_2, mail a@b.c <hover:show_text:'@x'>");
        let names: Vec<&str> = m.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["Steve", "alex_2"]);
        assert!(find_mentions("@").is_empty());
        assert!(find_mentions("@aaaaaaaaaaaaaaaaaaaa").is_empty()); // 20 chars: too long
        assert_eq!(find_mentions("\\@x @y")[0].name, "y");
    }

    #[test]
    fn mentions_are_replaced_when_resolved() {
        let (out, names) = apply_mentions(
            "hey @steve, @nobody and @STEVE",
            "<yellow>@{name}</yellow>",
            |n| n.eq_ignore_ascii_case("steve").then(|| "Steve".to_owned()),
        );
        assert_eq!(
            out,
            "hey <yellow>@Steve</yellow>, @nobody and <yellow>@Steve</yellow>"
        );
        assert_eq!(names, vec!["Steve".to_owned()]);
    }

    #[test]
    fn interactive_name_renders() {
        let mini = interactive_name("Alex", "<gray>Group: <white>vip", "/msg Alex ");
        let json = Text::mini(&mini).to_json();
        assert!(json.contains("suggest_command"), "{json}");
        assert!(json.contains("/msg Alex "), "{json}");
        assert!(json.contains("show_text"), "{json}");
        assert_eq!(Text::mini(&mini).plain(), "Alex");
        assert_eq!(interactive_name("A<b", "", ""), "A\\<b");
    }
}
