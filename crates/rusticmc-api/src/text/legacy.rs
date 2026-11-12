//! `&`-style legacy formatting codes.

use super::{Color, Decoration, NamedColor, Style, Text};

/// Parses `input` with any of `markers` as the code character.
pub(super) fn parse(input: &str, markers: &[char]) -> Text {
    let chars: Vec<char> = input.chars().collect();
    let mut parts: Vec<Text> = Vec::new();
    let mut style = Style::default();
    let mut buf = String::new();
    let mut saw_code = false;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if markers.contains(&c)
            && i + 1 < chars.len()
            && let Some((new_style, consumed)) = code_at(&chars, i, markers, &style)
        {
            flush(&mut parts, &mut buf, &style);
            style = new_style;
            saw_code = true;
            i += consumed;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut parts, &mut buf, &style);

    if !saw_code {
        return Text::new(input);
    }
    Text::empty().extend(parts)
}

fn flush(parts: &mut Vec<Text>, buf: &mut String, style: &Style) {
    if buf.is_empty() {
        return;
    }
    let text = std::mem::take(buf);
    // Merge with the previous part when the style did not really change (e.g. `&a&a`).
    if let Some(last) = parts.last_mut()
        && last.style() == style
        && last.children().is_empty()
        && last.literal_text().is_some()
    {
        last.push_literal(&text);
        return;
    }
    parts.push(Text::new(text).with_style(style.clone()));
}

/// The style after the code at `chars[i]` (a marker), and how many chars it used.
fn code_at(chars: &[char], i: usize, markers: &[char], current: &Style) -> Option<(Style, usize)> {
    let code = chars.get(i + 1)?.to_ascii_lowercase();
    if let Some(color) = NamedColor::from_code(code) {
        return Some((color_style(Color::Named(color)), 2));
    }
    let deco = match code {
        'k' => Some(Decoration::Obfuscated),
        'l' => Some(Decoration::Bold),
        'm' => Some(Decoration::Strikethrough),
        'n' => Some(Decoration::Underlined),
        'o' => Some(Decoration::Italic),
        _ => None,
    };
    if let Some(d) = deco {
        let mut s = current.clone();
        s.set(d, true);
        return Some((s, 2));
    }
    match code {
        'r' => Some((Style::default(), 2)),
        '#' => {
            // &#RRGGBB
            let hex: String = chars.get(i + 2..i + 8)?.iter().collect();
            let v = u32::from_str_radix(&hex, 16).ok()?;
            hex.chars()
                .all(|c| c.is_ascii_hexdigit())
                .then(|| (color_style(Color::Hex(v)), 8))
        }
        'x' => {
            // &x&R&R&G&G&B&B
            let mut hex = String::new();
            for k in 0..6 {
                let m = *chars.get(i + 2 + k * 2)?;
                let d = *chars.get(i + 3 + k * 2)?;
                if !markers.contains(&m) || !d.is_ascii_hexdigit() {
                    return None;
                }
                hex.push(d);
            }
            let v = u32::from_str_radix(&hex, 16).ok()?;
            Some((color_style(Color::Hex(v)), 14))
        }
        _ => None,
    }
}

fn color_style(c: Color) -> Style {
    Style {
        color: Some(c),
        ..Style::default()
    }
}

/// Removes legacy codes (`&` and `§`) from `input`, keeping the text.
pub fn strip_legacy(input: &str) -> String {
    let markers = ['&', '§'];
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let dummy = Style::default();
    while i < chars.len() {
        if markers.contains(&chars[i])
            && let Some((_, n)) = code_at(&chars, i, &markers, &dummy)
        {
            i += n;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(t: &Text) -> serde_json::Value {
        serde_json::from_str(&t.to_json()).unwrap()
    }

    #[test]
    fn no_codes_is_plain() {
        assert_eq!(Text::legacy("Tom & Jerry"), Text::new("Tom & Jerry"));
        assert_eq!(Text::legacy("100%&"), Text::new("100%&"));
    }

    #[test]
    fn colors_and_decorations() {
        let t = Text::legacy("&aGreen &lbold&r plain §cred");
        let v = json(&t);
        assert_eq!(v["text"], "");
        let extra = v["extra"].as_array().unwrap();
        assert_eq!(extra[0]["text"], "Green ");
        assert_eq!(extra[0]["color"], "green");
        assert_eq!(extra[1]["text"], "bold");
        assert_eq!(extra[1]["color"], "green");
        assert_eq!(extra[1]["bold"], true);
        assert_eq!(extra[2]["text"], " plain ");
        assert!(extra[2].get("color").is_none());
        assert_eq!(extra[3]["text"], "red");
        assert_eq!(extra[3]["color"], "red");
        assert_eq!(t.plain(), "Green bold plain red");
    }

    #[test]
    fn color_resets_decorations() {
        let t = Text::legacy("&l&obi&9blue");
        let v = json(&t);
        assert_eq!(v["extra"][0]["bold"], true);
        assert_eq!(v["extra"][0]["italic"], true);
        assert_eq!(v["extra"][1]["color"], "blue");
        assert!(v["extra"][1].get("bold").is_none());
    }

    #[test]
    fn hex_forms() {
        let v = json(&Text::legacy("&#FF8800orange&x&0&0&f&f&0&0green"));
        assert_eq!(v["extra"][0]["color"], "#FF8800");
        assert_eq!(v["extra"][0]["text"], "orange");
        assert_eq!(v["extra"][1]["color"], "#00FF00");
        assert_eq!(v["extra"][1]["text"], "green");
        // Bad hex stays literal.
        let t = Text::legacy("&#GG0000x");
        assert_eq!(t.plain(), "&#GG0000x");
    }

    #[test]
    fn uppercase_codes_and_merging() {
        let t = Text::legacy("&Aa&ab");
        assert_eq!(json(&t)["extra"].as_array().unwrap().len(), 1);
        assert_eq!(t.plain(), "ab");
    }

    #[test]
    fn custom_marker_and_strip() {
        let t = Text::legacy_with("%cred &cnot", '%');
        assert_eq!(t.plain(), "red &cnot");
        assert_eq!(
            strip_legacy("&a&lHi §x§1§2§3§4§5§6there &zok"),
            "Hi there &zok"
        );
    }
}
