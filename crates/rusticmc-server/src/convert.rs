//! Conversions between Horizon's mod API types and Rustic's model (docs/ARCHITECTURE.md §5).
use horizon_client::mod_api::{Hand, NativeServerPlayer, Text as HText};
use rusticmc_core::model::{self, BlockPos, GameMode, Location, Text, Uuid};

pub fn uuid(v: u128) -> Uuid {
    Uuid::from_u128(v)
}

pub fn raw(u: Uuid) -> u128 {
    u.as_u128()
}

/// A Rustic text (component JSON) as Horizon's: the spans for its own use (the log, `plain`), the
/// exact JSON for the clients.
pub fn to_horizon(text: &Text) -> HText {
    match serde_json::from_str::<serde_json::Value>(text.as_json()) {
        Ok(json) => HText::literal(text.to_plain()).with_component(json),
        Err(_) => HText::literal(text.to_plain()),
    }
}

/// A Horizon text as Rustic's component JSON.
pub fn from_horizon(text: &HText) -> Text {
    Text::from_json_unchecked(text.component().to_string())
}

pub fn game_mode(mode: i32) -> GameMode {
    match mode {
        1 => GameMode::Creative,
        2 => GameMode::Adventure,
        3 => GameMode::Spectator,
        _ => GameMode::Survival,
    }
}

pub fn game_mode_id(mode: GameMode) -> i32 {
    match mode {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Adventure => 2,
        GameMode::Spectator => 3,
    }
}

pub fn location(p: &NativeServerPlayer) -> Location {
    Location {
        world: p.dimension.clone(),
        x: p.position[0],
        y: p.position[1],
        z: p.position[2],
        yaw: p.yaw,
        pitch: p.pitch,
    }
}

pub fn block_pos(pos: [i32; 3]) -> BlockPos {
    BlockPos {
        x: pos[0],
        y: pos[1],
        z: pos[2],
    }
}

pub fn hand(hand: Option<Hand>) -> model::Hand {
    match hand {
        Some(Hand::Off) => model::Hand::OffHand,
        _ => model::Hand::MainHand,
    }
}

/// `minecraft:oak_stairs[facing=north,half=bottom]` as Rustic's block state.
pub fn block_state(text: &str) -> model::BlockState {
    let (id, props) = match text.split_once('[') {
        Some((id, rest)) => (id, rest.trim_end_matches(']')),
        None => (text, ""),
    };
    model::BlockState {
        id: id.to_owned(),
        properties: props
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
    }
}

/// Rustic's block state as `id[k=v,...]`.
pub fn block_state_string(state: &model::BlockState) -> String {
    if state.properties.is_empty() {
        return state.id.clone();
    }
    let props: Vec<String> = state
        .properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    format!("{}[{}]", state.id, props.join(","))
}

/// A Horizon stack as Rustic's: the components as SNBT (empty for an item's defaults).
pub fn stack_to_core(stack: Option<&horizon_client::items::ItemStack>) -> model::ItemStack {
    let Some(s) = stack.filter(|s| s.count > 0 && &*s.item != "minecraft:air") else {
        return model::ItemStack::empty();
    };
    let components = horizon_client::item_data::encode_components(s)
        .map(|tag| horizon_world::snbt_text::to_snbt(&tag))
        .unwrap_or_default();
    model::ItemStack {
        id: s.item.to_string(),
        count: s.count,
        components,
    }
}

/// Rustic's stack as Horizon's: None for an empty one; a stack with components goes through the
/// game's own SNBT reader (`Host::item_from_snbt`), which checks the item and the components.
pub fn stack_to_horizon(
    host: &dyn horizon_client::mod_api::Host,
    stack: &model::ItemStack,
) -> Option<horizon_client::items::ItemStack> {
    if stack.is_empty() {
        return None;
    }
    if stack.components.trim().is_empty() {
        return Some(horizon_client::items::ItemStack::new(
            &stack.id,
            stack.count,
        ));
    }
    host.item_from_snbt(&format!(
        "{{id:\"{}\",count:{},components:{}}}",
        stack.id, stack.count, stack.components
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_states_round_trip() {
        let s = "minecraft:oak_stairs[facing=north,half=bottom]";
        let state = block_state(s);
        assert_eq!(state.id, "minecraft:oak_stairs");
        assert_eq!(state.properties.len(), 2);
        assert_eq!(block_state_string(&state), s);
        assert_eq!(block_state("minecraft:stone").properties, vec![]);
    }

    #[test]
    fn uuids_round_trip() {
        let v = 0x069a79f4_44e9_4726_a5be_fca90e38aaf5_u128;
        assert_eq!(raw(uuid(v)), v);
    }

    #[test]
    fn texts_keep_their_json() {
        let t = Text::from_json_unchecked(r#"{"text":"hi","color":"red"}"#);
        let h = to_horizon(&t);
        assert_eq!(h.plain(), "hi");
        let back: serde_json::Value = serde_json::from_str(from_horizon(&h).as_json()).unwrap();
        assert_eq!(back, serde_json::json!({"text": "hi", "color": "red"}));
    }
}
