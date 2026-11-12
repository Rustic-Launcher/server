//! Item stacks: a builder over the WIT `item-stack` record (id, count, components SNBT).
//!
//! ```
//! use rusticmc_api::{ItemStack, Text, text::NamedColor};
//!
//! let sword = ItemStack::new("diamond_sword")
//!     .name(Text::new("Excalibur").color(NamedColor::Gold))
//!     .lore_line(Text::new("Pulled from a stone"))
//!     .enchant("sharpness", 5)
//!     .unbreakable();
//! assert_eq!(sword.id(), "minecraft:diamond_sword");
//! assert!(sword.components_snbt().contains("\"minecraft:enchantments\":{\"minecraft:sharpness\":5}"));
//! ```
//!
//! Components are written in the data-component format of current versions: text components as
//! inline compounds (JSON objects are valid SNBT), enchantments as an `id -> level` map.
//! Use [`ItemStack::component`] for anything without a helper.

use std::collections::BTreeMap;

use crate::bindings::rustic::api::types::ItemStack as RawItemStack;
use crate::text::{Decoration, Text, json_str};
use crate::types::namespaced;

/// An item stack. Cheap to clone; converts to and from the WIT record.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemStack {
    id: String,
    count: u32,
    /// Components SNBT received from the host, kept verbatim (`{...}` or empty).
    base: String,
    /// Components set through the builder: key -> SNBT value.
    components: BTreeMap<String, String>,
    enchantments: BTreeMap<String, u32>,
}

const ENCHANTMENTS: &str = "minecraft:enchantments";

impl ItemStack {
    /// One item of `id`; a missing namespace means `minecraft:`.
    pub fn new(id: impl Into<String>) -> ItemStack {
        ItemStack {
            id: namespaced(id.into()),
            count: 1,
            ..ItemStack::default()
        }
    }

    /// The empty stack (air, count 0).
    pub fn empty() -> ItemStack {
        ItemStack {
            id: "minecraft:air".to_owned(),
            count: 0,
            ..ItemStack::default()
        }
    }

    /// Sets the count.
    pub fn count(mut self, count: u32) -> ItemStack {
        self.count = count;
        self
    }

    /// The custom name (`minecraft:custom_name`). Unless the text sets `italic` itself, it is
    /// made non-italic (vanilla shows renamed items in italics).
    pub fn name(self, name: impl Into<Text>) -> ItemStack {
        let t = non_italic(name.into());
        self.component("minecraft:custom_name", t.to_json())
    }

    /// The item name (`minecraft:item_name`): not italic, not removable in an anvil.
    pub fn item_name(self, name: impl Into<Text>) -> ItemStack {
        let t = name.into();
        self.component("minecraft:item_name", t.to_json())
    }

    /// Replaces the lore lines (non-italic unless the line sets `italic`).
    pub fn lore<I, T>(self, lines: I) -> ItemStack
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        let items: Vec<String> = lines
            .into_iter()
            .map(|l| non_italic(l.into()).to_json())
            .collect();
        self.component("minecraft:lore", format!("[{}]", items.join(",")))
    }

    /// Appends one lore line to the lines set through this builder.
    pub fn lore_line(mut self, line: impl Into<Text>) -> ItemStack {
        let json = non_italic(line.into()).to_json();
        let value = match self.components.remove("minecraft:lore") {
            Some(existing) if existing.len() > 2 => {
                format!("{},{}]", &existing[..existing.len() - 1], json)
            }
            _ => format!("[{json}]"),
        };
        self.components.insert("minecraft:lore".to_owned(), value);
        self
    }

    /// Adds an enchantment (`sharpness` or `minecraft:sharpness`) at `level`.
    pub fn enchant(mut self, enchantment: impl Into<String>, level: u32) -> ItemStack {
        self.enchantments
            .insert(namespaced(enchantment.into()), level);
        self
    }

    /// Makes the item unbreakable.
    pub fn unbreakable(self) -> ItemStack {
        self.component("minecraft:unbreakable", "{}")
    }

    /// Forces the enchantment glint on or off.
    pub fn glint(self, on: bool) -> ItemStack {
        self.component("minecraft:enchantment_glint_override", on.to_string())
    }

    /// Overrides the maximum stack size (1..=99).
    pub fn max_stack_size(self, size: u8) -> ItemStack {
        self.component("minecraft:max_stack_size", size.clamp(1, 99).to_string())
    }

    /// Custom model data as a list of floats (resource packs select models with it).
    pub fn custom_model_data(self, value: f32) -> ItemStack {
        self.component(
            "minecraft:custom_model_data",
            format!("{{\"floats\":[{value:?}f]}}"),
        )
    }

    /// Sets any data component to an SNBT value: `.component("minecraft:rarity", "\"epic\"")`.
    pub fn component(mut self, key: impl Into<String>, snbt_value: impl Into<String>) -> ItemStack {
        self.components
            .insert(namespaced(key.into()), snbt_value.into());
        self
    }

    /// Removes a component set through this builder.
    pub fn without_component(mut self, key: &str) -> ItemStack {
        self.components.remove(&namespaced(key.to_owned()));
        self
    }

    /// The item id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The count.
    pub fn amount(&self) -> u32 {
        self.count
    }

    /// Whether the stack is empty (count 0 or air).
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.id == "minecraft:air"
    }

    /// The level of an enchantment set through this builder.
    pub fn enchantment(&self, id: &str) -> Option<u32> {
        self.enchantments.get(&namespaced(id.to_owned())).copied()
    }

    /// The components patch as SNBT: what the host received from the server, with the
    /// builder's components appended (a later key overrides an earlier one).
    pub fn components_snbt(&self) -> String {
        let mut entries: Vec<String> = Vec::new();
        for (k, v) in &self.components {
            let mut e = String::new();
            json_str(&mut e, k);
            e.push(':');
            e.push_str(v);
            entries.push(e);
        }
        if !self.enchantments.is_empty() {
            let mut e = String::new();
            json_str(&mut e, ENCHANTMENTS);
            e.push_str(":{");
            for (i, (id, lvl)) in self.enchantments.iter().enumerate() {
                if i > 0 {
                    e.push(',');
                }
                json_str(&mut e, id);
                e.push(':');
                e.push_str(&lvl.to_string());
            }
            e.push('}');
            entries.push(e);
        }
        let base = self.base.trim();
        let base_inner = base
            .strip_prefix('{')
            .and_then(|b| b.strip_suffix('}'))
            .map(str::trim)
            .unwrap_or("");
        match (base_inner.is_empty(), entries.is_empty()) {
            (true, true) => String::new(),
            (true, false) => format!("{{{}}}", entries.join(",")),
            (false, true) => format!("{{{base_inner}}}"),
            (false, false) => format!("{{{base_inner},{}}}", entries.join(",")),
        }
    }

    /// The WIT record.
    pub fn to_raw(&self) -> RawItemStack {
        RawItemStack {
            id: self.id.clone(),
            count: self.count,
            components: self.components_snbt(),
        }
    }

    /// From the WIT record (components are kept verbatim).
    pub fn from_raw(raw: RawItemStack) -> ItemStack {
        ItemStack {
            id: raw.id,
            count: raw.count,
            base: raw.components,
            components: BTreeMap::new(),
            enchantments: BTreeMap::new(),
        }
    }
}

fn non_italic(t: Text) -> Text {
    if t.style().italic.is_some() {
        t
    } else {
        t.decorate(Decoration::Italic, false)
    }
}

impl From<ItemStack> for RawItemStack {
    fn from(i: ItemStack) -> Self {
        i.to_raw()
    }
}

impl From<&ItemStack> for RawItemStack {
    fn from(i: &ItemStack) -> Self {
        i.to_raw()
    }
}

impl From<RawItemStack> for ItemStack {
    fn from(r: RawItemStack) -> Self {
        ItemStack::from_raw(r)
    }
}

impl From<&str> for ItemStack {
    /// One item of that id.
    fn from(id: &str) -> Self {
        ItemStack::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_item() {
        let i = ItemStack::new("stone").count(16);
        let raw = i.to_raw();
        assert_eq!(raw.id, "minecraft:stone");
        assert_eq!(raw.count, 16);
        assert_eq!(raw.components, "");
        assert!(!i.is_empty());
        assert!(ItemStack::empty().is_empty());
    }

    #[test]
    fn components_snbt() {
        let i = ItemStack::new("minecraft:diamond_sword")
            .name("Blade")
            .lore(["one", "two"])
            .enchant("sharpness", 5)
            .enchant("minecraft:unbreaking", 3)
            .unbreakable()
            .glint(false)
            .component("rarity", "\"epic\"");
        let s = i.components_snbt();
        assert_eq!(
            s,
            concat!(
                "{\"minecraft:custom_name\":{\"text\":\"Blade\",\"italic\":false},",
                "\"minecraft:enchantment_glint_override\":false,",
                "\"minecraft:lore\":[{\"text\":\"one\",\"italic\":false},{\"text\":\"two\",\"italic\":false}],",
                "\"minecraft:rarity\":\"epic\",",
                "\"minecraft:unbreakable\":{},",
                "\"minecraft:enchantments\":{\"minecraft:sharpness\":5,\"minecraft:unbreaking\":3}}"
            )
        );
        assert_eq!(i.enchantment("sharpness"), Some(5));
    }

    #[test]
    fn italic_kept_when_set() {
        let s = ItemStack::new("paper")
            .name(Text::new("x").italic())
            .components_snbt();
        assert!(s.contains("\"italic\":true"));
    }

    #[test]
    fn lore_lines_accumulate() {
        let s = ItemStack::new("paper")
            .lore_line("a")
            .lore_line("b")
            .components_snbt();
        assert_eq!(
            s,
            "{\"minecraft:lore\":[{\"text\":\"a\",\"italic\":false},{\"text\":\"b\",\"italic\":false}]}"
        );
    }

    #[test]
    fn merges_with_host_components() {
        let raw = RawItemStack {
            id: "minecraft:bow".into(),
            count: 1,
            components: "{\"minecraft:damage\":3}".into(),
        };
        let i = ItemStack::from_raw(raw.clone());
        assert_eq!(i.to_raw(), raw);
        let i = i.unbreakable();
        assert_eq!(
            i.components_snbt(),
            "{\"minecraft:damage\":3,\"minecraft:unbreakable\":{}}"
        );
        let removed = i.without_component("unbreakable");
        assert_eq!(removed.components_snbt(), "{\"minecraft:damage\":3}");
    }

    #[test]
    fn misc_components() {
        let s = ItemStack::new("stick")
            .max_stack_size(200)
            .custom_model_data(2.0)
            .components_snbt();
        assert!(s.contains("\"minecraft:max_stack_size\":99"));
        assert!(s.contains("\"minecraft:custom_model_data\":{\"floats\":[2.0f]}"));
    }
}
