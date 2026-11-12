//! Text components: a builder producing vanilla JSON chat components, plus parsers for
//! `&`-legacy codes ([`Text::legacy`]) and a small MiniMessage-like tag syntax ([`Text::mini`]).
//!
//! ```
//! use rusticmc_api::text::{Text, NamedColor, ClickEvent};
//!
//! let t = Text::new("Click me")
//!     .color(NamedColor::Gold)
//!     .bold()
//!     .hover(Text::new("Teleports you to spawn"))
//!     .click(ClickEvent::RunCommand("/spawn".into()));
//! assert_eq!(
//!     t.to_json(),
//!     r#"{"text":"Click me","color":"gold","bold":true,"click_event":{"action":"run_command","command":"/spawn"},"hover_event":{"action":"show_text","value":{"text":"Teleports you to spawn"}}}"#
//! );
//!
//! let m = Text::mini("<red>Hello <bold>world</bold>!");
//! assert_eq!(m.plain(), "Hello world!");
//! ```
//!
//! The JSON uses the current field names (`click_event` / `hover_event`, `command` / `url` /
//! `value`), which the server also accepts in their older camel-case form.

mod legacy;
mod mini;

use std::fmt::Write as _;

pub use legacy::strip_legacy;
pub use mini::escape_mini;

/// The 16 named chat colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum NamedColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
}

impl NamedColor {
    /// All colors, in legacy code order (`0`..`f`).
    pub const ALL: [NamedColor; 16] = [
        NamedColor::Black,
        NamedColor::DarkBlue,
        NamedColor::DarkGreen,
        NamedColor::DarkAqua,
        NamedColor::DarkRed,
        NamedColor::DarkPurple,
        NamedColor::Gold,
        NamedColor::Gray,
        NamedColor::DarkGray,
        NamedColor::Blue,
        NamedColor::Green,
        NamedColor::Aqua,
        NamedColor::Red,
        NamedColor::LightPurple,
        NamedColor::Yellow,
        NamedColor::White,
    ];

    /// The JSON name (`dark_aqua`).
    pub const fn name(self) -> &'static str {
        match self {
            NamedColor::Black => "black",
            NamedColor::DarkBlue => "dark_blue",
            NamedColor::DarkGreen => "dark_green",
            NamedColor::DarkAqua => "dark_aqua",
            NamedColor::DarkRed => "dark_red",
            NamedColor::DarkPurple => "dark_purple",
            NamedColor::Gold => "gold",
            NamedColor::Gray => "gray",
            NamedColor::DarkGray => "dark_gray",
            NamedColor::Blue => "blue",
            NamedColor::Green => "green",
            NamedColor::Aqua => "aqua",
            NamedColor::Red => "red",
            NamedColor::LightPurple => "light_purple",
            NamedColor::Yellow => "yellow",
            NamedColor::White => "white",
        }
    }

    /// The legacy code character (`0`..`9`, `a`..`f`).
    pub fn code(self) -> char {
        let i = NamedColor::ALL
            .iter()
            .position(|c| *c == self)
            .unwrap_or(15);
        char::from_digit(i as u32, 16).unwrap_or('f')
    }

    /// The color for a legacy code character.
    pub fn from_code(c: char) -> Option<NamedColor> {
        c.to_digit(16).map(|d| NamedColor::ALL[d as usize])
    }

    /// The color for a name; accepts `grey` spellings.
    pub fn from_name(name: &str) -> Option<NamedColor> {
        let n = name.to_ascii_lowercase().replace("grey", "gray");
        NamedColor::ALL.into_iter().find(|c| c.name() == n)
    }

    /// The RGB value vanilla renders.
    pub const fn rgb(self) -> u32 {
        match self {
            NamedColor::Black => 0x000000,
            NamedColor::DarkBlue => 0x0000AA,
            NamedColor::DarkGreen => 0x00AA00,
            NamedColor::DarkAqua => 0x00AAAA,
            NamedColor::DarkRed => 0xAA0000,
            NamedColor::DarkPurple => 0xAA00AA,
            NamedColor::Gold => 0xFFAA00,
            NamedColor::Gray => 0xAAAAAA,
            NamedColor::DarkGray => 0x555555,
            NamedColor::Blue => 0x5555FF,
            NamedColor::Green => 0x55FF55,
            NamedColor::Aqua => 0x55FFFF,
            NamedColor::Red => 0xFF5555,
            NamedColor::LightPurple => 0xFF55FF,
            NamedColor::Yellow => 0xFFFF55,
            NamedColor::White => 0xFFFFFF,
        }
    }
}

/// A text color: one of the 16 named colors or any RGB value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    /// A named color.
    Named(NamedColor),
    /// `0xRRGGBB`.
    Hex(u32),
}

impl Color {
    /// Parses `red`, `dark_gray`/`dark_grey` or `#RRGGBB` (also `#RGB`).
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex).map(Color::Hex);
        }
        NamedColor::from_name(s).map(Color::Named)
    }

    /// An RGB color from components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::Hex(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    /// The JSON value: the name, or `#RRGGBB`.
    pub fn to_json_value(self) -> String {
        match self {
            Color::Named(n) => n.name().to_owned(),
            Color::Hex(v) => format!("#{:06X}", v & 0xFF_FFFF),
        }
    }
}

fn parse_hex(hex: &str) -> Option<u32> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
            Some(((r * 17) << 16) | ((g * 17) << 8) | (b * 17))
        }
        _ => None,
    }
}

impl From<NamedColor> for Color {
    fn from(c: NamedColor) -> Self {
        Color::Named(c)
    }
}

impl From<u32> for Color {
    /// `0xRRGGBB`.
    fn from(v: u32) -> Self {
        Color::Hex(v & 0xFF_FFFF)
    }
}

/// The five text decorations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum Decoration {
    Bold,
    Italic,
    Underlined,
    Strikethrough,
    Obfuscated,
}

impl Decoration {
    /// The JSON field name.
    pub const fn name(self) -> &'static str {
        match self {
            Decoration::Bold => "bold",
            Decoration::Italic => "italic",
            Decoration::Underlined => "underlined",
            Decoration::Strikethrough => "strikethrough",
            Decoration::Obfuscated => "obfuscated",
        }
    }
}

/// What happens when the text is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickEvent {
    /// Runs a command as the clicking player (include the leading `/`).
    RunCommand(String),
    /// Puts the text in the chat box.
    SuggestCommand(String),
    /// Opens a URL (http/https) after the client's confirmation.
    OpenUrl(String),
    /// Copies the text to the clipboard.
    CopyToClipboard(String),
    /// Turns to a page of a book.
    ChangePage(u32),
}

/// What shows when the text is hovered.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverEvent {
    /// A tooltip with text.
    ShowText(Box<Text>),
}

/// The style of a component. `None` fields inherit from the parent component.
#[derive(Debug, Clone, PartialEq, Default)]
#[allow(missing_docs)]
pub struct Style {
    pub color: Option<Color>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub font: Option<String>,
    pub insertion: Option<String>,
    pub click: Option<ClickEvent>,
    pub hover: Option<HoverEvent>,
}

impl Style {
    /// Whether no field is set.
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }

    /// Sets a decoration flag.
    pub fn set(&mut self, d: Decoration, on: bool) {
        *self.flag_mut(d) = Some(on);
    }

    /// A decoration flag.
    pub fn get(&self, d: Decoration) -> Option<bool> {
        match d {
            Decoration::Bold => self.bold,
            Decoration::Italic => self.italic,
            Decoration::Underlined => self.underlined,
            Decoration::Strikethrough => self.strikethrough,
            Decoration::Obfuscated => self.obfuscated,
        }
    }

    fn flag_mut(&mut self, d: Decoration) -> &mut Option<bool> {
        match d {
            Decoration::Bold => &mut self.bold,
            Decoration::Italic => &mut self.italic,
            Decoration::Underlined => &mut self.underlined,
            Decoration::Strikethrough => &mut self.strikethrough,
            Decoration::Obfuscated => &mut self.obfuscated,
        }
    }

    /// Overwrites this style's fields with the fields `top` sets.
    pub fn apply(&mut self, top: &Style) {
        if top.color.is_some() {
            self.color = top.color;
        }
        for d in [
            Decoration::Bold,
            Decoration::Italic,
            Decoration::Underlined,
            Decoration::Strikethrough,
            Decoration::Obfuscated,
        ] {
            if let Some(v) = top.get(d) {
                self.set(d, v);
            }
        }
        if top.font.is_some() {
            self.font.clone_from(&top.font);
        }
        if top.insertion.is_some() {
            self.insertion.clone_from(&top.insertion);
        }
        if top.click.is_some() {
            self.click.clone_from(&top.click);
        }
        if top.hover.is_some() {
            self.hover.clone_from(&top.hover);
        }
    }

    fn write_fields(&self, out: &mut String) {
        if let Some(c) = self.color {
            out.push_str(",\"color\":");
            json_str(out, &c.to_json_value());
        }
        for d in [
            Decoration::Bold,
            Decoration::Italic,
            Decoration::Underlined,
            Decoration::Strikethrough,
            Decoration::Obfuscated,
        ] {
            if let Some(v) = self.get(d) {
                let _ = write!(out, ",\"{}\":{}", d.name(), v);
            }
        }
        if let Some(font) = &self.font {
            out.push_str(",\"font\":");
            json_str(out, font);
        }
        if let Some(ins) = &self.insertion {
            out.push_str(",\"insertion\":");
            json_str(out, ins);
        }
        if let Some(click) = &self.click {
            out.push_str(",\"click_event\":{\"action\":");
            match click {
                ClickEvent::RunCommand(c) => {
                    out.push_str("\"run_command\",\"command\":");
                    json_str(out, c);
                }
                ClickEvent::SuggestCommand(c) => {
                    out.push_str("\"suggest_command\",\"command\":");
                    json_str(out, c);
                }
                ClickEvent::OpenUrl(u) => {
                    out.push_str("\"open_url\",\"url\":");
                    json_str(out, u);
                }
                ClickEvent::CopyToClipboard(v) => {
                    out.push_str("\"copy_to_clipboard\",\"value\":");
                    json_str(out, v);
                }
                ClickEvent::ChangePage(p) => {
                    let _ = write!(out, "\"change_page\",\"page\":{p}");
                }
            }
            out.push('}');
        }
        if let Some(HoverEvent::ShowText(t)) = &self.hover {
            out.push_str(",\"hover_event\":{\"action\":\"show_text\",\"value\":");
            t.write_json(out);
            out.push('}');
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Content {
    Literal(String),
    Translatable {
        key: String,
        args: Vec<Text>,
        fallback: Option<String>,
    },
    Keybind(String),
    /// A component given as JSON by the caller (or received from the host), kept verbatim.
    Json(String),
}

/// A chat component: content, style and children (which inherit the style).
///
/// Build it with the methods below, or parse it with [`Text::mini`] / [`Text::legacy`];
/// [`Text::to_json`] gives the vanilla JSON the host expects for every `text` parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    content: Content,
    style: Style,
    children: Vec<Text>,
}

impl Default for Text {
    fn default() -> Self {
        Text::empty()
    }
}

impl Text {
    /// Literal text.
    pub fn new(text: impl Into<String>) -> Text {
        Text {
            content: Content::Literal(text.into()),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Empty text, usually a parent for [`Text::append`].
    pub fn empty() -> Text {
        Text::new("")
    }

    /// A translatable component (`"translate"`), rendered in the client's language:
    /// `Text::translatable("item.minecraft.diamond")`.
    pub fn translatable(key: impl Into<String>) -> Text {
        Text {
            content: Content::Translatable {
                key: key.into(),
                args: Vec::new(),
                fallback: None,
            },
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// A key binding name (`"keybind"`), such as `key.jump`.
    pub fn keybind(key: impl Into<String>) -> Text {
        Text {
            content: Content::Keybind(key.into()),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// A component given as JSON, used verbatim (not validated). Style and children added
    /// afterwards wrap it.
    pub fn from_json(json: impl Into<String>) -> Text {
        Text {
            content: Content::Json(json.into()),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Parses the MiniMessage-like tag syntax: `<red>`, `<#ff8800>`, `<bold>`/`<b>`, `<!italic>`,
    /// `<reset>`, `<hover:show_text:'...'>`, `<click:run_command:'/spawn'>`, `<insert:'..'>`,
    /// `<lang:key:arg>`, `<key:key.jump>`, `<newline>`. Closing tags `</red>`, `</>`.
    /// Unknown or malformed tags are kept as literal text; `\<` escapes a `<`.
    pub fn mini(input: &str) -> Text {
        mini::parse(input)
    }

    /// Parses legacy formatting codes with `&` or `§`: `&a` colors, `&l` bold, `&o` italic,
    /// `&n` underlined, `&m` strikethrough, `&k` obfuscated, `&r` reset, plus hex colors
    /// `&#RRGGBB` and `&x&R&R&G&G&B&B`. A color code resets decorations, as in vanilla.
    pub fn legacy(input: &str) -> Text {
        legacy::parse(input, &['&', '§'])
    }

    /// Like [`Text::legacy`] with a single marker character.
    pub fn legacy_with(input: &str, marker: char) -> Text {
        legacy::parse(input, &[marker])
    }

    /// Adds an argument to a translatable component (`"with"`); ignored on other content.
    pub fn arg(mut self, arg: impl Into<Text>) -> Text {
        if let Content::Translatable { args, .. } = &mut self.content {
            args.push(arg.into());
        }
        self
    }

    /// Fallback text for a translatable component whose key the client does not know.
    pub fn fallback(mut self, text: impl Into<String>) -> Text {
        if let Content::Translatable { fallback, .. } = &mut self.content {
            *fallback = Some(text.into());
        }
        self
    }

    /// Sets the color.
    pub fn color(mut self, color: impl Into<Color>) -> Text {
        self.style.color = Some(color.into());
        self
    }

    /// Sets a decoration on (`true`) or explicitly off (`false`, e.g. to cancel the default
    /// italic of item lore).
    pub fn decorate(mut self, d: Decoration, on: bool) -> Text {
        self.style.set(d, on);
        self
    }

    /// Bold.
    pub fn bold(self) -> Text {
        self.decorate(Decoration::Bold, true)
    }

    /// Italic.
    pub fn italic(self) -> Text {
        self.decorate(Decoration::Italic, true)
    }

    /// Explicitly not italic.
    pub fn not_italic(self) -> Text {
        self.decorate(Decoration::Italic, false)
    }

    /// Underlined.
    pub fn underlined(self) -> Text {
        self.decorate(Decoration::Underlined, true)
    }

    /// Strikethrough.
    pub fn strikethrough(self) -> Text {
        self.decorate(Decoration::Strikethrough, true)
    }

    /// Obfuscated (scrambling glyphs).
    pub fn obfuscated(self) -> Text {
        self.decorate(Decoration::Obfuscated, true)
    }

    /// The resource-pack font to use (`minecraft:uniform`).
    pub fn font(mut self, font: impl Into<String>) -> Text {
        self.style.font = Some(font.into());
        self
    }

    /// Text inserted into the chat box on shift-click.
    pub fn insertion(mut self, text: impl Into<String>) -> Text {
        self.style.insertion = Some(text.into());
        self
    }

    /// A tooltip.
    pub fn hover(mut self, text: impl Into<Text>) -> Text {
        self.style.hover = Some(HoverEvent::ShowText(Box::new(text.into())));
        self
    }

    /// A click action.
    pub fn click(mut self, click: ClickEvent) -> Text {
        self.style.click = Some(click);
        self
    }

    /// Click runs `command` (with its `/`).
    pub fn run_command(self, command: impl Into<String>) -> Text {
        self.click(ClickEvent::RunCommand(command.into()))
    }

    /// Click puts `command` into the chat box.
    pub fn suggest_command(self, command: impl Into<String>) -> Text {
        self.click(ClickEvent::SuggestCommand(command.into()))
    }

    /// Click opens `url`.
    pub fn open_url(self, url: impl Into<String>) -> Text {
        self.click(ClickEvent::OpenUrl(url.into()))
    }

    /// Click copies `value` to the clipboard.
    pub fn copy_to_clipboard(self, value: impl Into<String>) -> Text {
        self.click(ClickEvent::CopyToClipboard(value.into()))
    }

    /// Replaces the whole style.
    pub fn with_style(mut self, style: Style) -> Text {
        self.style = style;
        self
    }

    /// Adds a child component (inherits this component's style).
    pub fn append(mut self, child: impl Into<Text>) -> Text {
        self.children.push(child.into());
        self
    }

    /// Adds several children.
    pub fn extend(mut self, children: impl IntoIterator<Item = Text>) -> Text {
        self.children.extend(children);
        self
    }

    /// The style.
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// The style, mutably.
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    /// The children.
    pub fn children(&self) -> &[Text] {
        &self.children
    }

    /// Whether this renders nothing (empty literal, no children).
    pub fn is_empty(&self) -> bool {
        matches!(&self.content, Content::Literal(s) if s.is_empty()) && self.children.is_empty()
    }

    /// The text without formatting: literals concatenated, translation keys and key binds by
    /// name. Verbatim JSON content is read for its `text` fields.
    pub fn plain(&self) -> String {
        let mut out = String::new();
        self.write_plain(&mut out);
        out
    }

    fn write_plain(&self, out: &mut String) {
        match &self.content {
            Content::Literal(s) => out.push_str(s),
            Content::Translatable { key, fallback, .. } => {
                out.push_str(fallback.as_deref().unwrap_or(key));
            }
            Content::Keybind(k) => out.push_str(k),
            Content::Json(j) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(j) {
                    plain_from_value(&v, out);
                }
            }
        }
        for c in &self.children {
            c.write_plain(out);
        }
    }

    /// The vanilla JSON component.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        if let Content::Json(raw) = &self.content {
            if self.style.is_empty() && self.children.is_empty() {
                out.push_str(raw);
                return;
            }
            out.push_str("{\"text\":\"\"");
            self.style.write_fields(out);
            out.push_str(",\"extra\":[");
            out.push_str(raw);
            for c in &self.children {
                out.push(',');
                c.write_json(out);
            }
            out.push_str("]}");
            return;
        }
        out.push('{');
        match &self.content {
            Content::Literal(s) => {
                out.push_str("\"text\":");
                json_str(out, s);
            }
            Content::Translatable {
                key,
                args,
                fallback,
            } => {
                out.push_str("\"translate\":");
                json_str(out, key);
                if let Some(f) = fallback {
                    out.push_str(",\"fallback\":");
                    json_str(out, f);
                }
                if !args.is_empty() {
                    out.push_str(",\"with\":[");
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        a.write_json(out);
                    }
                    out.push(']');
                }
            }
            Content::Keybind(k) => {
                out.push_str("\"keybind\":");
                json_str(out, k);
            }
            Content::Json(_) => unreachable!(),
        }
        self.style.write_fields(out);
        if !self.children.is_empty() {
            out.push_str(",\"extra\":[");
            for (i, c) in self.children.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                c.write_json(out);
            }
            out.push(']');
        }
        out.push('}');
    }

    pub(crate) fn literal_text(&self) -> Option<&str> {
        match &self.content {
            Content::Literal(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn push_literal(&mut self, s: &str) {
        if let Content::Literal(t) = &mut self.content {
            t.push_str(s);
        }
    }
}

fn plain_from_value(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::String(s) => out.push_str(s),
        serde_json::Value::Array(items) => items.iter().for_each(|i| plain_from_value(i, out)),
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(s)) = map.get("text") {
                out.push_str(s);
            } else if let Some(serde_json::Value::String(s)) = map.get("translate") {
                out.push_str(s);
            }
            if let Some(extra) = map.get("extra") {
                plain_from_value(extra, out);
            }
        }
        _ => {}
    }
}

/// Writes `s` as a JSON string literal.
pub(crate) fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<&str> for Text {
    /// Literal text (not parsed: use [`Text::mini`] or [`Text::legacy`] for markup).
    fn from(s: &str) -> Self {
        Text::new(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::new(s)
    }
}

impl From<&String> for Text {
    fn from(s: &String) -> Self {
        Text::new(s.as_str())
    }
}

impl From<&Text> for Text {
    fn from(t: &Text) -> Self {
        t.clone()
    }
}

impl From<Text> for String {
    /// The JSON form, what WIT `text` parameters take.
    fn from(t: Text) -> Self {
        t.to_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("valid JSON")
    }

    #[test]
    fn literal_and_escaping() {
        assert_eq!(Text::new("hi").to_json(), r#"{"text":"hi"}"#);
        let t = Text::new("a\"b\\c\nd\u{1}");
        assert_eq!(t.to_json(), r#"{"text":"a\"b\\c\nd\u0001"}"#);
        assert_eq!(parse(&t.to_json())["text"], "a\"b\\c\nd\u{1}");
    }

    #[test]
    fn styles_and_events() {
        let t = Text::new("x")
            .color(Color::rgb(0x12, 0xab, 0xff))
            .bold()
            .not_italic()
            .underlined()
            .strikethrough()
            .obfuscated()
            .font("minecraft:uniform")
            .insertion("ins")
            .open_url("https://example.org")
            .hover(Text::new("tip").color(NamedColor::Gray));
        let v = parse(&t.to_json());
        assert_eq!(v["color"], "#12ABFF");
        assert_eq!(v["bold"], true);
        assert_eq!(v["italic"], false);
        assert_eq!(v["underlined"], true);
        assert_eq!(v["strikethrough"], true);
        assert_eq!(v["obfuscated"], true);
        assert_eq!(v["font"], "minecraft:uniform");
        assert_eq!(v["insertion"], "ins");
        assert_eq!(v["click_event"]["action"], "open_url");
        assert_eq!(v["click_event"]["url"], "https://example.org");
        assert_eq!(v["hover_event"]["action"], "show_text");
        assert_eq!(v["hover_event"]["value"]["text"], "tip");
        assert_eq!(v["hover_event"]["value"]["color"], "gray");
    }

    #[test]
    fn click_actions() {
        let cases = [
            (
                Text::new("").run_command("/spawn"),
                "run_command",
                "command",
                "/spawn",
            ),
            (
                Text::new("").suggest_command("/msg "),
                "suggest_command",
                "command",
                "/msg ",
            ),
            (
                Text::new("").copy_to_clipboard("42"),
                "copy_to_clipboard",
                "value",
                "42",
            ),
        ];
        for (t, action, field, value) in cases {
            let v = parse(&t.to_json());
            assert_eq!(v["click_event"]["action"], action);
            assert_eq!(v["click_event"][field], value);
        }
        let v = parse(&Text::new("").click(ClickEvent::ChangePage(3)).to_json());
        assert_eq!(v["click_event"]["page"], 3);
    }

    #[test]
    fn children_translatable_keybind() {
        let t = Text::new("Hello ")
            .color(NamedColor::Gold)
            .append(Text::new("world").bold())
            .append(
                Text::translatable("chat.type.text")
                    .arg("Steve")
                    .arg(Text::new("hi").italic())
                    .fallback("<%s> %s"),
            )
            .append(Text::keybind("key.jump"));
        let v = parse(&t.to_json());
        assert_eq!(v["extra"][0]["text"], "world");
        assert_eq!(v["extra"][1]["translate"], "chat.type.text");
        assert_eq!(v["extra"][1]["fallback"], "<%s> %s");
        assert_eq!(v["extra"][1]["with"][0]["text"], "Steve");
        assert_eq!(v["extra"][1]["with"][1]["italic"], true);
        assert_eq!(v["extra"][2]["keybind"], "key.jump");
        assert_eq!(t.plain(), "Hello world<%s> %skey.jump");
    }

    #[test]
    fn raw_json_content() {
        let raw = r#"{"text":"raw","color":"red"}"#;
        assert_eq!(Text::from_json(raw).to_json(), raw);
        let wrapped = Text::from_json(raw).bold().append("!");
        let v = parse(&wrapped.to_json());
        assert_eq!(v["bold"], true);
        assert_eq!(v["extra"][0]["text"], "raw");
        assert_eq!(v["extra"][1]["text"], "!");
        assert_eq!(wrapped.plain(), "raw!");
    }

    #[test]
    fn colors() {
        assert_eq!(
            Color::parse("dark_grey"),
            Some(Color::Named(NamedColor::DarkGray))
        );
        assert_eq!(Color::parse("#FF8800"), Some(Color::Hex(0xFF8800)));
        assert_eq!(Color::parse("#f80"), Some(Color::Hex(0xFF8800)));
        assert_eq!(Color::parse("#ggg"), None);
        assert_eq!(Color::parse("nope"), None);
        assert_eq!(NamedColor::from_code('a'), Some(NamedColor::Green));
        assert_eq!(NamedColor::Green.code(), 'a');
        assert_eq!(NamedColor::Black.code(), '0');
        assert_eq!(
            Color::Named(NamedColor::LightPurple).to_json_value(),
            "light_purple"
        );
        assert_eq!(Color::from(0x1234567).to_json_value(), "#234567");
    }

    #[test]
    fn into_conversions() {
        let s: String = Text::new("a").into();
        assert_eq!(s, r#"{"text":"a"}"#);
        let t: Text = "b".into();
        assert_eq!(t.plain(), "b");
        assert!(Text::empty().is_empty());
        assert!(!Text::empty().append("x").is_empty());
    }
}
