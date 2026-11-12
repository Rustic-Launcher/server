//! Calls from the JVM into Rustic: what the Java layer's API objects need (players, worlds, the
//! server, permissions, commands, event subscriptions). They arrive while the core is calling one
//! of the bridge's plugin instances, so the runtime and the backend of that call are in
//! [`rusticmc_core::context`].

use crate::bridge::Bridge;
use rusticmc_core::context;
use rusticmc_core::model::{
    BlockPos, BlockState, CommandSender, GameMode, HostError, Location, PermissionDefault,
    Priority, Text, Uuid,
};
use rusticmc_core::{Backend, CommandOutcome, PluginId, Rustic};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn arg<T: DeserializeOwned>(req: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(req.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("`{key}`: {e}"))
}

fn text(req: &Value, key: &str) -> Result<Text, String> {
    let s: String = arg(req, key)?;
    Ok(Text::from_json_unchecked(s))
}

fn host_err(e: HostError) -> String {
    format!("{e:?}")
}

fn game<T>(f: impl FnOnce(&mut dyn Backend) -> Result<T, HostError>) -> Result<T, String> {
    context::backend_call(f).map_err(host_err)
}

fn to_json<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn runtime() -> Result<Rustic, String> {
    context::runtime().map_err(host_err)
}

fn bridge_api() -> Result<rusticmc_core::PluginApi, String> {
    Ok(runtime()?.api(PluginId::new_unchecked(crate::descriptor::BRIDGE_ID)))
}

fn priority(s: &str) -> Priority {
    match s {
        "lowest" => Priority::Lowest,
        "low" => Priority::Low,
        "high" => Priority::High,
        "highest" => Priority::Highest,
        "monitor" => Priority::Monitor,
        _ => Priority::Normal,
    }
}

impl Bridge {
    fn plugin_id(&self, req: &Value) -> Result<PluginId, String> {
        let name: String = arg(req, "plugin")?;
        self.id_of(&name)
            .ok_or_else(|| format!("unknown Bukkit plugin `{name}`"))
    }

    /// Serve one request of the JVM.
    pub(crate) fn serve(&self, req: &Value) -> Result<Value, String> {
        let op = req.get("op").and_then(Value::as_str).unwrap_or("");
        match op {
            "subscribe" => {
                let id = self.plugin_id(req)?;
                let event: String = arg(req, "event")?;
                let prio: String = arg(req, "priority")?;
                let handler =
                    runtime()?
                        .api(id.clone())
                        .subscribe(&event, priority(&prio), false)?;
                self.remember_handler(&id, handler, &prio);
                Ok(json!(handler))
            }
            "has_permission" => {
                let sender: CommandSender = arg(req, "sender")?;
                let node: String = arg(req, "node")?;
                Ok(json!(bridge_api()?.has_permission(sender, &node)))
            }
            "declare_permission" => {
                let id = self.plugin_id(req)?;
                let node: String = arg(req, "node")?;
                let default: String = arg(req, "default").unwrap_or_default();
                let description: String = arg(req, "description").unwrap_or_default();
                let children: Vec<(String, bool)> = arg(req, "children").unwrap_or_default();
                runtime()?.api(id).declare_permission(
                    &node,
                    crate::descriptor::permission_default(&default),
                    &description,
                    &children,
                )?;
                Ok(Value::Null)
            }
            "disable_plugin" => {
                let id = self.plugin_id(req)?;
                self.queue_disable(id);
                Ok(Value::Null)
            }
            "dispatch_command" => {
                let sender: CommandSender = arg(req, "sender")?;
                let line: String = arg(req, "line")?;
                let line = line.trim_start_matches('/').to_owned();
                let rt = runtime()?;
                let outcome = context::backend_mut(|b| rt.dispatch_command(sender, &line, b))
                    .map_err(host_err)?;
                Ok(json!(match outcome {
                    CommandOutcome::UnknownCommand if sender == CommandSender::Console => {
                        game(|b| b.server_dispatch_command(&line))?
                    }
                    CommandOutcome::UnknownCommand => false,
                    _ => true,
                }))
            }
            "send_message" => {
                let to: CommandSender = arg(req, "to")?;
                let msg = text(req, "text")?;
                match to {
                    CommandSender::Console => game(|b| b.console_message(&msg))?,
                    CommandSender::Player(p) => game(|b| b.players_send_message(p, &msg))?,
                }
                Ok(Value::Null)
            }
            "broadcast" => {
                let msg = text(req, "text")?;
                let perm: Option<String> = arg(req, "permission").unwrap_or(None);
                match perm {
                    None => game(|b| b.server_broadcast(&msg))?,
                    Some(node) => {
                        let api = bridge_api()?;
                        for p in game(|b| b.players_online())? {
                            if api.has_permission(CommandSender::Player(p), &node) {
                                game(|b| b.players_send_message(p, &msg))?;
                            }
                        }
                        game(|b| b.console_message(&msg))?;
                    }
                }
                Ok(Value::Null)
            }
            "player" => self.player_get(req),
            "player_set" => self.player_set(req),
            "teleport" => {
                let p: Uuid = arg(req, "uuid")?;
                let to: Location = arg(req, "to")?;
                game(|b| b.players_teleport(p, &to))?;
                Ok(Value::Null)
            }
            "kick" => {
                let p: Uuid = arg(req, "uuid")?;
                let reason = text(req, "text")?;
                game(|b| b.players_kick(p, &reason))?;
                Ok(Value::Null)
            }
            "title" => {
                let p: Uuid = arg(req, "uuid")?;
                let (t, s) = (text(req, "title")?, text(req, "subtitle")?);
                let (fi, st, fo): (u32, u32, u32) = (
                    arg(req, "fade_in")?,
                    arg(req, "stay")?,
                    arg(req, "fade_out")?,
                );
                game(|b| b.players_send_title(p, &t, &s, fi, st, fo))?;
                Ok(Value::Null)
            }
            "action_bar" => {
                let p: Uuid = arg(req, "uuid")?;
                let t = text(req, "text")?;
                game(|b| b.players_send_action_bar(p, &t))?;
                Ok(Value::Null)
            }
            "sound" => {
                let p: Uuid = arg(req, "uuid")?;
                let sound: String = arg(req, "sound")?;
                let (v, pitch): (f32, f32) = (arg(req, "volume")?, arg(req, "pitch")?);
                game(|b| b.players_play_sound(p, &sound, v, pitch))?;
                Ok(Value::Null)
            }
            "server" => {
                let what: String = arg(req, "get")?;
                match what.as_str() {
                    "version" => to_json(game(|b| b.server_version())?),
                    "max_players" => to_json(game(|b| b.server_max_players())?),
                    "motd" => to_json(game(|b| b.server_motd())?),
                    "tick_stats" => to_json(game(|b| b.server_get_tick_stats())?),
                    "operators" => to_json(game(|b| b.server_operators())?),
                    "worlds" => to_json(game(|b| b.worlds_all())?),
                    "online" => to_json(game(|b| b.players_online())?),
                    _ => Err(format!("unknown server value `{what}`")),
                }
            }
            "offline_player" => {
                let name: String = arg(req, "name")?;
                to_json(game(|b| b.server_offline_player(&name)).unwrap_or(None))
            }
            "world" => {
                let w: String = arg(req, "world")?;
                let what: String = arg(req, "get")?;
                match what.as_str() {
                    "time" => to_json(game(|b| b.worlds_time(&w))?),
                    "spawn" => to_json(game(|b| b.worlds_spawn(&w))?),
                    "players" => to_json(game(|b| b.worlds_players(&w))?),
                    "highest_y" => {
                        let (x, z): (i32, i32) = (arg(req, "x")?, arg(req, "z")?);
                        to_json(game(|b| b.worlds_highest_block_y(&w, x, z))?)
                    }
                    _ => Err(format!("unknown world value `{what}`")),
                }
            }
            "world_set" => {
                let w: String = arg(req, "world")?;
                let t: u64 = arg(req, "value")?;
                game(|b| b.worlds_set_time(&w, t))?;
                Ok(Value::Null)
            }
            "block" => {
                let w: String = arg(req, "world")?;
                let pos: BlockPos = arg(req, "pos")?;
                to_json(game(|b| b.worlds_get_block(&w, pos))?)
            }
            "set_block" => {
                let w: String = arg(req, "world")?;
                let pos: BlockPos = arg(req, "pos")?;
                let state = BlockState {
                    id: arg(req, "id")?,
                    properties: Vec::new(),
                };
                let physics: bool = arg(req, "physics").unwrap_or(true);
                to_json(game(|b| b.worlds_set_block(&w, pos, &state, physics))?)
            }
            _ => Err(format!("unknown host call `{op}`")),
        }
    }

    fn player_get(&self, req: &Value) -> Result<Value, String> {
        let p: Uuid = arg(req, "uuid")?;
        let what: String = arg(req, "get")?;
        match what.as_str() {
            "health" => to_json(game(|b| b.players_health(p))? as f64),
            "max_health" => to_json(game(|b| b.players_max_health(p))? as f64),
            "food" => to_json(game(|b| b.players_food_level(p))?),
            "saturation" => to_json(game(|b| b.players_saturation(p))? as f64),
            "level" => to_json(game(|b| b.players_experience_level(p))?),
            "game_mode" => to_json(game(|b| b.players_get_game_mode(p))?),
            "location" => to_json(game(|b| b.players_get_location(p))?),
            "ping" => to_json(game(|b| b.players_ping(p))?),
            "address" => to_json(game(|b| b.players_address(p))?),
            "locale" => to_json(game(|b| b.players_locale(p))?),
            "op" => to_json(game(|b| b.players_is_op(p))?),
            "online" => to_json(game(|b| b.players_is_online(p))?),
            _ => Err(format!("unknown player value `{what}`")),
        }
    }

    fn player_set(&self, req: &Value) -> Result<Value, String> {
        let p: Uuid = arg(req, "uuid")?;
        let what: String = arg(req, "set")?;
        match what.as_str() {
            "health" => {
                let v: f32 = arg(req, "value")?;
                game(|b| b.players_set_health(p, v))?
            }
            "food" => {
                let v: u32 = arg(req, "value")?;
                game(|b| b.players_set_food_level(p, v))?
            }
            "saturation" => {
                let v: f32 = arg(req, "value")?;
                game(|b| b.players_set_saturation(p, v))?
            }
            "level" => {
                let v: u32 = arg(req, "value")?;
                game(|b| b.players_set_experience_level(p, v))?
            }
            "game_mode" => {
                let v: GameMode = arg(req, "value")?;
                game(|b| b.players_set_game_mode(p, v))?
            }
            "flying" => {
                let allow: bool = arg(req, "value")?;
                let flying: bool = arg(req, "flying").unwrap_or(false);
                game(|b| b.players_set_flying(p, allow, flying))?
            }
            "op" => {
                let v: bool = arg(req, "value")?;
                game(|b| b.players_set_op(p, v))?
            }
            _ => return Err(format!("unknown player setting `{what}`")),
        }
        Ok(Value::Null)
    }
}

/// Permission defaults from plugin.yml, declared when a plugin is enabled.
pub(crate) fn declare_permissions(
    rt: &Rustic,
    id: &PluginId,
    perms: &[crate::descriptor::PermissionDesc],
) {
    let api = rt.api(id.clone());
    for p in perms {
        let d: PermissionDefault = crate::descriptor::permission_default(&p.default);
        let _ = api.declare_permission(&p.name, d, &p.description, &p.children);
    }
}
