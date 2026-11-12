//! Player-facing messages in English and French (`lang = "en" | "fr"` in `config.toml`).
//!
//! Templates are MiniMessage with `{name}` placeholders; [`fill`] escapes the values, so player
//! names, home names and chat text can never inject tags. Palette: gray text, yellow values,
//! green confirmations, red errors, all behind the same `[Ess]` header.

use rustic::Text;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};

/// The message language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// English.
    #[default]
    En,
    /// French.
    Fr,
}

/// The header of every plugin message.
pub const HEADER: &str = "<dark_gray>[<gold>Ess</gold>]</dark_gray> ";

macro_rules! messages {
    ($($key:ident => $en:literal, $fr:literal;)*) => {
        /// A message key.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Msg { $(
            #[allow(missing_docs)]
            $key
        ),* }

        impl Msg {
            /// Every key (for tests).
            pub const ALL: &'static [Msg] = &[$(Msg::$key),*];

            /// The template in a language.
            pub fn template(self, lang: Lang) -> &'static str {
                match (self, lang) {
                    $( (Msg::$key, Lang::En) => $en, (Msg::$key, Lang::Fr) => $fr, )*
                }
            }
        }
    };
}

messages! {
    PlayersOnly => "<red>Only players can use this command.",
        "<red>Seuls les joueurs peuvent utiliser cette commande.";
    InvalidName => "<red>Invalid name <yellow>{name}</yellow> (1 to 32 characters among a-z 0-9 _ -).",
        "<red>Nom invalide <yellow>{name}</yellow> (1 à 32 caractères parmi a-z 0-9 _ -).";

    // Teleports
    TeleportingIn => "<gray>Teleporting to {place} in <yellow>{seconds}s</yellow>. Don't move!",
        "<gray>Téléportation vers {place} dans <yellow>{seconds}s</yellow>. Ne bougez pas !";
    TeleportCancelled => "<red>Teleport cancelled: you moved.",
        "<red>Téléportation annulée : vous avez bougé.";
    TeleportReplaced => "<gray>Your previous pending teleport was cancelled.",
        "<gray>Votre téléportation en attente a été annulée.";
    Teleported => "<green>Teleported to {place}.",
        "<green>Téléporté vers {place}.";
    Cooldown => "<red>Wait <yellow>{seconds}s</yellow> before teleporting again.",
        "<red>Attendez <yellow>{seconds}s</yellow> avant de vous téléporter à nouveau.";
    TeleportFailed => "<red>Teleport failed: {error}",
        "<red>La téléportation a échoué : {error}";
    PlaceSpawn => "<yellow>spawn</yellow>", "<yellow>l'apparition</yellow>";
    PlaceHome => "home <yellow>{name}</yellow>", "la résidence <yellow>{name}</yellow>";
    PlaceWarp => "warp <yellow>{name}</yellow>", "le point <yellow>{name}</yellow>";
    PlaceBack => "<yellow>your previous location</yellow>", "<yellow>votre position précédente</yellow>";
    PlacePlayer => "<yellow>{name}</yellow>", "<yellow>{name}</yellow>";

    // Spawn
    SpawnSet => "<green>Spawn set at <yellow>{x} {y} {z}</yellow> in <yellow>{world}</yellow>.",
        "<green>Point d'apparition défini en <yellow>{x} {y} {z}</yellow> dans <yellow>{world}</yellow>.";
    NoSpawn => "<red>No spawn is set and the world spawn is unavailable.",
        "<red>Aucun point d'apparition défini et celui du monde est indisponible.";

    // Homes
    HomeSet => "<green>Home <yellow>{name}</yellow> set.",
        "<green>Résidence <yellow>{name}</yellow> définie.";
    HomeMoved => "<green>Home <yellow>{name}</yellow> moved here.",
        "<green>Résidence <yellow>{name}</yellow> déplacée ici.";
    HomeLimit => "<red>You can have at most <yellow>{max}</yellow> homes. Delete one with <yellow>/delhome</yellow>.",
        "<red>Vous pouvez avoir au plus <yellow>{max}</yellow> résidences. Supprimez-en une avec <yellow>/delhome</yellow>.";
    HomeCharged => "<gray><yellow>{cost}</yellow> was charged for this home.",
        "<gray><yellow>{cost}</yellow> ont été prélevés pour cette résidence.";
    HomeTooExpensive => "<red>A new home costs <yellow>{cost}</yellow>: {reason}",
        "<red>Une nouvelle résidence coûte <yellow>{cost}</yellow> : {reason}";
    HomeDeleted => "<green>Home <yellow>{name}</yellow> deleted.",
        "<green>Résidence <yellow>{name}</yellow> supprimée.";
    UnknownHome => "<red>You have no home named <yellow>{name}</yellow>.",
        "<red>Vous n'avez aucune résidence nommée <yellow>{name}</yellow>.";
    NoHomes => "<gray>You have no home yet. Set one with <yellow>/sethome</yellow>.",
        "<gray>Vous n'avez encore aucune résidence. Définissez-en une avec <yellow>/sethome</yellow>.";
    HomesList => "<gray>Your homes (<yellow>{count}</yellow>/<yellow>{max}</yellow>), click one:",
        "<gray>Vos résidences (<yellow>{count}</yellow>/<yellow>{max}</yellow>), cliquez :";
    Unlimited => "∞", "∞";
    ClickToTeleport => "<gray>Click to teleport to <yellow>{name}</yellow>\n<dark_gray>{world} {x} {y} {z}",
        "<gray>Cliquez pour aller à <yellow>{name}</yellow>\n<dark_gray>{world} {x} {y} {z}";

    // Warps
    WarpSet => "<green>Warp <yellow>{name}</yellow> set.",
        "<green>Point <yellow>{name}</yellow> défini.";
    WarpDeleted => "<green>Warp <yellow>{name}</yellow> deleted.",
        "<green>Point <yellow>{name}</yellow> supprimé.";
    UnknownWarp => "<red>There is no warp named <yellow>{name}</yellow>.",
        "<red>Il n'existe aucun point nommé <yellow>{name}</yellow>.";
    NoWarpPermission => "<red>You may not use warp <yellow>{name}</yellow>.",
        "<red>Vous ne pouvez pas utiliser le point <yellow>{name}</yellow>.";
    NoWarps => "<gray>There are no warps yet.",
        "<gray>Il n'y a encore aucun point de téléportation.";
    WarpsList => "<gray>Warps (<yellow>{count}</yellow>), click one:",
        "<gray>Points de téléportation (<yellow>{count}</yellow>), cliquez :";

    // Teleport requests
    TpaSelf => "<red>You cannot send a request to yourself.",
        "<red>Vous ne pouvez pas vous envoyer une demande.";
    TpaSent => "<green>Request sent to <yellow>{name}</yellow>. It expires in <yellow>{seconds}s</yellow>.",
        "<green>Demande envoyée à <yellow>{name}</yellow>. Elle expire dans <yellow>{seconds}s</yellow>.";
    TpaReceived => "<yellow>{name}</yellow> <gray>wants to teleport to you.",
        "<yellow>{name}</yellow> <gray>veut se téléporter vers vous.";
    TpaHereReceived => "<yellow>{name}</yellow> <gray>wants you to teleport to them.",
        "<yellow>{name}</yellow> <gray>veut que vous vous téléportiez vers lui.";
    TpaButtons => "<green><hover:show_text:'<gray>Accept the request'><click:run_command:'/tpaccept {name}'>[Accept]</click></hover></green> <red><hover:show_text:'<gray>Deny the request'><click:run_command:'/tpdeny {name}'>[Deny]</click></hover></red> <dark_gray>({seconds}s)",
        "<green><hover:show_text:'<gray>Accepter la demande'><click:run_command:'/tpaccept {name}'>[Accepter]</click></hover></green> <red><hover:show_text:'<gray>Refuser la demande'><click:run_command:'/tpdeny {name}'>[Refuser]</click></hover></red> <dark_gray>({seconds}s)";
    TpaNone => "<red>You have no pending teleport request.",
        "<red>Vous n'avez aucune demande de téléportation en attente.";
    TpaNoneFrom => "<red>No pending request from <yellow>{name}</yellow>.",
        "<red>Aucune demande en attente de <yellow>{name}</yellow>.";
    TpaAccepted => "<green>You accepted the request from <yellow>{name}</yellow>.",
        "<green>Vous avez accepté la demande de <yellow>{name}</yellow>.";
    TpaAcceptedOther => "<green><yellow>{name}</yellow> accepted your request.",
        "<green><yellow>{name}</yellow> a accepté votre demande.";
    TpaDenied => "<gray>You denied the request from <yellow>{name}</yellow>.",
        "<gray>Vous avez refusé la demande de <yellow>{name}</yellow>.";
    TpaDeniedOther => "<red><yellow>{name}</yellow> denied your request.",
        "<red><yellow>{name}</yellow> a refusé votre demande.";
    TpaExpired => "<gray>Your teleport request to <yellow>{name}</yellow> expired.",
        "<gray>Votre demande de téléportation à <yellow>{name}</yellow> a expiré.";
    TpaExpiredTarget => "<gray>The teleport request from <yellow>{name}</yellow> expired.",
        "<gray>La demande de téléportation de <yellow>{name}</yellow> a expiré.";
    PlayerOffline => "<red><yellow>{name}</yellow> is not online any more.",
        "<red><yellow>{name}</yellow> n'est plus en ligne.";

    // Back
    BackNone => "<red>There is no previous location to return to.",
        "<red>Aucune position précédente où revenir.";
    BackOnDeath => "<gray>Use <yellow><click:run_command:'/back'><hover:show_text:'<gray>/back'>/back</hover></click></yellow> to return to where you died.",
        "<gray>Utilisez <yellow><click:run_command:'/back'><hover:show_text:'<gray>/back'>/back</hover></click></yellow> pour revenir où vous êtes mort.";

    // Messages
    MsgSelf => "<red>You cannot message yourself.",
        "<red>Vous ne pouvez pas vous écrire à vous-même.";
    MsgNoReply => "<red>Nobody to reply to.",
        "<red>Personne à qui répondre.";
    Me => "me", "moi";
    MsgFormat => "<gray>[{from} <dark_gray>→</dark_gray> {to}<gray>]</gray> <white>{message}",
        "<gray>[{from} <dark_gray>→</dark_gray> {to}<gray>]</gray> <white>{message}";
    MsgHover => "<gray>Click to reply", "<gray>Cliquez pour répondre";
}

/// Fills a template: every `{key}` is replaced with the escaped value.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), &escape_mini(v));
    }
    out
}

/// Fills a template with already-MiniMessage values (not escaped).
pub fn fill_raw(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// A message with the header.
pub fn msg(lang: Lang, key: Msg, args: &[(&str, &str)]) -> Text {
    Text::mini(&format!("{HEADER}{}", fill(key.template(lang), args)))
}

/// A message with the header, escaped `args` and already-MiniMessage `raw` values.
pub fn msg_with(lang: Lang, key: Msg, args: &[(&str, &str)], raw: &[(&str, &str)]) -> Text {
    Text::mini(&format!(
        "{HEADER}{}",
        fill_raw(&fill(key.template(lang), args), raw)
    ))
}

/// A template filled with escaped values, as MiniMessage source (to nest in another message).
pub fn part(lang: Lang, key: Msg, args: &[(&str, &str)]) -> String {
    fill(key.template(lang), args)
}

/// Converts chat meta (legacy `&`/`§` codes or MiniMessage) to MiniMessage source, so it can be
/// spliced into a template.
pub fn meta_to_mini(raw: &str) -> String {
    if !looks_legacy(raw) {
        return raw.to_owned();
    }
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if (c == '&' || c == '§')
            && let Some(&code) = chars.peek()
        {
            let tag = match code.to_ascii_lowercase() {
                '0' => Some("<black>"),
                '1' => Some("<dark_blue>"),
                '2' => Some("<dark_green>"),
                '3' => Some("<dark_aqua>"),
                '4' => Some("<dark_red>"),
                '5' => Some("<dark_purple>"),
                '6' => Some("<gold>"),
                '7' => Some("<gray>"),
                '8' => Some("<dark_gray>"),
                '9' => Some("<blue>"),
                'a' => Some("<green>"),
                'b' => Some("<aqua>"),
                'c' => Some("<red>"),
                'd' => Some("<light_purple>"),
                'e' => Some("<yellow>"),
                'f' => Some("<white>"),
                'k' => Some("<obfuscated>"),
                'l' => Some("<bold>"),
                'm' => Some("<strikethrough>"),
                'n' => Some("<underlined>"),
                'o' => Some("<italic>"),
                'r' => Some("<reset>"),
                _ => None,
            };
            if let Some(tag) = tag {
                chars.next();
                out.push_str(tag);
                continue;
            }
            if code == '#' {
                let hex: String = chars.clone().skip(1).take(6).collect();
                if hex.len() == 6 && hex.chars().all(|h| h.is_ascii_hexdigit()) {
                    for _ in 0..7 {
                        chars.next();
                    }
                    out.push_str(&format!("<#{hex}>"));
                    continue;
                }
            }
        }
        if c == '<' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Whether text uses legacy formatting codes (`&a`, `§l`, `&#RRGGBB`).
pub fn looks_legacy(raw: &str) -> bool {
    let chars: Vec<char> = raw.chars().collect();
    chars.windows(2).any(|w| {
        (w[0] == '&' || w[0] == '§') && (w[1].is_ascii_hexdigit() || "klmnorKLMNOR#".contains(w[1]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holes(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s
            .split('{')
            .skip(1)
            .filter_map(|p| p.split_once('}').map(|(k, _)| k.to_owned()))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn every_message_has_both_languages_and_same_placeholders() {
        for m in Msg::ALL {
            let (en, fr) = (m.template(Lang::En), m.template(Lang::Fr));
            assert!(!en.is_empty() && !fr.is_empty(), "{m:?}");
            assert_eq!(holes(en), holes(fr), "{m:?}");
        }
    }

    #[test]
    fn values_are_escaped() {
        let t = msg(Lang::En, Msg::HomeSet, &[("name", "<red>x")]);
        assert_eq!(t.plain(), "[Ess] Home <red>x set.");
        let t = msg(Lang::Fr, Msg::HomeDeleted, &[("name", "base")]);
        assert_eq!(t.plain(), "[Ess] Résidence base supprimée.");
    }

    #[test]
    fn buttons_have_click_events() {
        let t = msg(
            Lang::En,
            Msg::TpaButtons,
            &[("name", "Alice"), ("seconds", "60")],
        );
        let json = t.to_json();
        assert!(json.contains("/tpaccept Alice"), "{json}");
        assert!(json.contains("/tpdeny Alice"), "{json}");
        assert_eq!(t.plain(), "[Ess] [Accept] [Deny] (60s)");
    }

    #[test]
    fn legacy_meta_converts_to_mini() {
        assert_eq!(meta_to_mini("&6[VIP] "), "<gold>[VIP] ");
        assert_eq!(meta_to_mini("&l&#FF8800X"), "<bold><#FF8800>X");
        assert_eq!(meta_to_mini("<red>[A]"), "<red>[A]");
        assert_eq!(meta_to_mini("&a<b>"), "<green>\\<b>");
        assert_eq!(Text::mini(&meta_to_mini("&6[VIP]")).plain(), "[VIP]");
    }
}
