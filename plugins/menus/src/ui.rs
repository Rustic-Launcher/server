//! Reusable menu building blocks (pure: item builders, layout and pagination math). The README
//! "recipe book" refers to these.

use rustic::{ItemStack, Text, Uuid};

use crate::lang::{Lang, Msg, tr};

/// Slots per row.
pub const ROW: u32 = 9;

/// The slot of `(row, column)`, both from 0.
pub const fn slot(row: u32, column: u32) -> u32 {
    row * ROW + column
}

/// Number of pages for `total` entries, `per_page` per page (at least 1).
pub fn page_count(total: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 1;
    }
    total.div_ceil(per_page).max(1)
}

/// Clamps a 0-based page to the valid range.
pub fn clamp_page(page: usize, total: usize, per_page: usize) -> usize {
    page.min(page_count(total, per_page) - 1)
}

/// The index range of `page` (0-based, clamped).
pub fn page_range(total: usize, per_page: usize, page: usize) -> std::ops::Range<usize> {
    let page = clamp_page(page, total, per_page);
    let start = page * per_page;
    start..(start + per_page).min(total)
}

/// Rows needed to show `items` entries inside a one-slot border (7 per row), plus the border
/// rows (2), clamped to 3..=6.
pub fn rows_for_bordered(items: usize) -> u8 {
    let inner = items.div_ceil(7).max(1);
    (inner + 2).clamp(3, 6) as u8
}

/// The `i`-th slot inside a one-slot border (columns 1..=7, from row 1).
pub fn bordered_slot(i: usize) -> u32 {
    let (row, col) = (i / 7, i % 7);
    slot(row as u32 + 1, col as u32 + 1)
}

/// The UUID as the four big-endian ints of the profile component (`[I;a,b,c,d]`).
pub fn uuid_ints(uuid: Uuid) -> [i32; 4] {
    let v = uuid.as_u128();
    [
        (v >> 96) as u32 as i32,
        (v >> 64) as u32 as i32,
        (v >> 32) as u32 as i32,
        v as u32 as i32,
    ]
}

/// The `minecraft:profile` component value (SNBT) of a player head showing `name`'s skin.
pub fn profile_snbt(name: &str, uuid: Option<Uuid>) -> String {
    let mut name_json = String::new();
    for c in name.chars() {
        match c {
            '"' | '\\' => {
                name_json.push('\\');
                name_json.push(c);
            }
            c if c.is_control() => {}
            c => name_json.push(c),
        }
    }
    match uuid {
        Some(u) => {
            let [a, b, c, d] = uuid_ints(u);
            format!("{{name:\"{name_json}\",id:[I;{a},{b},{c},{d}]}}")
        }
        None => format!("{{name:\"{name_json}\"}}"),
    }
}

/// A player head item for `name` (the client fetches the skin from the profile).
pub fn player_head(name: &str, uuid: Option<Uuid>) -> ItemStack {
    ItemStack::new("minecraft:player_head").component("minecraft:profile", profile_snbt(name, uuid))
}

/// A named filler pane (borders, backgrounds).
pub fn filler() -> ItemStack {
    ItemStack::new("minecraft:gray_stained_glass_pane").name(" ")
}

/// A button: an icon with a MiniMessage-like name and lore lines.
pub fn button(icon: &str, name: &str, lore: &[String]) -> ItemStack {
    ItemStack::new(icon)
        .name(Text::mini(name))
        .lore(lore.iter().map(|l| Text::mini(l)))
}

/// The "back" arrow.
pub fn back_button(lang: Lang) -> ItemStack {
    ItemStack::new("minecraft:arrow").name(tr(lang, Msg::Back, &[]))
}

/// The "close" barrier.
pub fn close_button(lang: Lang) -> ItemStack {
    ItemStack::new("minecraft:barrier").name(tr(lang, Msg::Close, &[]))
}

/// A toggle: the item's name colour, lore state line and enchantment glint follow `on`.
pub fn toggle_item(
    icon: &str,
    name: &str,
    description: &[String],
    on: bool,
    lang: Lang,
) -> ItemStack {
    let state = if on {
        crate::lang::tr_mini(lang, Msg::On, &[])
    } else {
        crate::lang::tr_mini(lang, Msg::Off, &[])
    };
    let color = if on { "<green>" } else { "<red>" };
    let mut lore: Vec<Text> = description.iter().map(|l| Text::mini(l)).collect();
    lore.push(Text::new(""));
    lore.push(Text::mini(
        &crate::lang::template(lang, Msg::State).replace("{state}", &state),
    ));
    lore.push(tr(lang, Msg::ToggleHints, &[]));
    ItemStack::new(icon)
        .name(Text::mini(&format!(
            "{color}{}",
            rustic::text::escape_mini(name)
        )))
        .lore(lore)
        .glint(on)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination() {
        assert_eq!(page_count(0, 45), 1);
        assert_eq!(page_count(45, 45), 1);
        assert_eq!(page_count(46, 45), 2);
        assert_eq!(page_range(100, 45, 0), 0..45);
        assert_eq!(page_range(100, 45, 2), 90..100);
        assert_eq!(page_range(100, 45, 9), 90..100); // clamped
        assert_eq!(page_range(0, 45, 3), 0..0);
        assert_eq!(clamp_page(5, 10, 5), 1);
        assert_eq!(page_count(3, 0), 1);
    }

    #[test]
    fn layout() {
        assert_eq!(slot(2, 4), 22);
        assert_eq!(rows_for_bordered(0), 3);
        assert_eq!(rows_for_bordered(7), 3);
        assert_eq!(rows_for_bordered(8), 4);
        assert_eq!(rows_for_bordered(100), 6);
        assert_eq!(bordered_slot(0), 10);
        assert_eq!(bordered_slot(6), 16);
        assert_eq!(bordered_slot(7), 19);
    }

    #[test]
    fn heads() {
        let u: Uuid = "069a79f4-44e9-4726-a5be-fca90e38aaf5".parse().unwrap();
        assert_eq!(
            uuid_ints(u),
            [0x069a79f4, 0x44e94726, 0xa5befca9_u32 as i32, 0x0e38aaf5]
        );
        assert_eq!(
            profile_snbt("Notch", Some(u)),
            "{name:\"Notch\",id:[I;110787060,1156138790,-1514210135,238594805]}"
        );
        assert_eq!(profile_snbt("a\"b", None), "{name:\"a\\\"b\"}");
        let head = player_head("Notch", None);
        assert_eq!(head.id(), "minecraft:player_head");
        assert_eq!(
            head.components_snbt(),
            "{\"minecraft:profile\":{name:\"Notch\"}}"
        );
    }

    #[test]
    fn toggles_reflect_state() {
        let on = toggle_item(
            "minecraft:lime_dye",
            "Sounds",
            &["<gray>Plays sounds".into()],
            true,
            Lang::En,
        );
        let off = toggle_item("minecraft:lime_dye", "Sounds", &[], false, Lang::En);
        let (s_on, s_off) = (on.components_snbt(), off.components_snbt());
        assert!(
            s_on.contains("\"minecraft:enchantment_glint_override\":true"),
            "{s_on}"
        );
        assert!(
            s_off.contains("\"minecraft:enchantment_glint_override\":false"),
            "{s_off}"
        );
        assert!(s_on.contains("Enabled") && s_off.contains("Disabled"));
        assert!(s_on.contains("\"color\":\"green\"") && s_off.contains("\"color\":\"red\""));
        let fr = toggle_item("minecraft:lime_dye", "Sons", &[], true, Lang::Fr).components_snbt();
        assert!(fr.contains("Activé"), "{fr}");
    }
}
