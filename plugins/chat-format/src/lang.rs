//! Player-facing messages in English and French (MiniMessage-like templates, `{placeholders}`
//! filled with escaped values).

use rustic::Text;

use crate::format::fill;

/// A language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English.
    En,
    /// French.
    Fr,
}

impl Lang {
    /// From a config value (`en`, `fr`) or a client locale (`fr_fr`, `fr_ca`, `en_us`...).
    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase();
        if c == "fr" || c.starts_with("fr_") || c.starts_with("fr-") {
            Some(Lang::Fr)
        } else if c == "en" || c.starts_with("en_") || c.starts_with("en-") {
            Some(Lang::En)
        } else {
            None
        }
    }
}

/// Message keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Msg {
    MutedPerm,
    MutedTemp,
    TooFast,
    Duplicate,
    Caps,
    Link,
    Word,
    MuteDonePerm,
    MuteDoneTemp,
    YouWereMutedPerm,
    YouWereMutedTemp,
    MuteNotify,
    Unmuted,
    NotMuted,
    YouWereUnmuted,
    UnknownPlayer,
    CannotMute,
    MuteListHeader,
    MuteListEntry,
    MuteListEmpty,
    Permanent,
    Mentioned,
    Reloaded,
    ReloadFailed,
    CommandBlockedMuted,
}

/// The template of a message.
pub fn template(lang: Lang, msg: Msg) -> &'static str {
    use Msg::*;
    match lang {
        Lang::En => match msg {
            MutedPerm => "<red>You are muted.</red> <gray>{reason}",
            MutedTemp => "<red>You are muted for <white>{time}</white>.</red> <gray>{reason}",
            TooFast => "<red>Slow down! Wait <white>{seconds}s</white> before chatting again.",
            Duplicate => "<red>Please do not repeat the same message.",
            Caps => "<red>Please do not shout (too many capital letters).",
            Link => "<red>Links are not allowed in chat.",
            Word => "<red>Your message contains a forbidden word.",
            MuteDonePerm => {
                "<green>Muted <white>{player}</white> permanently.</green> <gray>{reason}"
            }
            MuteDoneTemp => {
                "<green>Muted <white>{player}</white> for <white>{time}</white>.</green> <gray>{reason}"
            }
            YouWereMutedPerm => "<red>You have been muted by {by}.</red> <gray>{reason}",
            YouWereMutedTemp => {
                "<red>You have been muted for <white>{time}</white> by {by}.</red> <gray>{reason}"
            }
            MuteNotify => {
                "<dark_gray>[<gray>Staff</gray>]</dark_gray> <gray>{by} muted {player} ({time}). {reason}"
            }
            Unmuted => "<green><white>{player}</white> can chat again.",
            NotMuted => "<red>{player} is not muted.",
            YouWereUnmuted => "<green>You can chat again.",
            UnknownPlayer => "<red>Unknown player: {player}",
            CannotMute => "<red>{player} cannot be muted.",
            MuteListHeader => "<gold>Muted players ({count}):",
            MuteListEntry => {
                "<dark_gray>-</dark_gray> <white>{player}</white> <dark_gray>({time})</dark_gray> <gray>{reason}"
            }
            MuteListEmpty => "<gray>Nobody is muted.",
            Permanent => "permanent",
            Mentioned => "<yellow>{player} mentioned you",
            Reloaded => "<green>chat-format configuration reloaded.",
            ReloadFailed => "<red>Reload failed: {error}",
            CommandBlockedMuted => "<red>You cannot use this command while muted.",
        },
        Lang::Fr => match msg {
            MutedPerm => "<red>Vous êtes réduit au silence.</red> <gray>{reason}",
            MutedTemp => {
                "<red>Vous êtes réduit au silence pour <white>{time}</white>.</red> <gray>{reason}"
            }
            TooFast => {
                "<red>Doucement ! Attendez <white>{seconds} s</white> avant d'écrire à nouveau."
            }
            Duplicate => "<red>Merci de ne pas répéter le même message.",
            Caps => "<red>Merci de ne pas crier (trop de majuscules).",
            Link => "<red>Les liens ne sont pas autorisés dans le chat.",
            Word => "<red>Votre message contient un mot interdit.",
            MuteDonePerm => {
                "<green><white>{player}</white> est réduit au silence définitivement.</green> <gray>{reason}"
            }
            MuteDoneTemp => {
                "<green><white>{player}</white> est réduit au silence pour <white>{time}</white>.</green> <gray>{reason}"
            }
            YouWereMutedPerm => "<red>{by} vous a réduit au silence.</red> <gray>{reason}",
            YouWereMutedTemp => {
                "<red>{by} vous a réduit au silence pour <white>{time}</white>.</red> <gray>{reason}"
            }
            MuteNotify => {
                "<dark_gray>[<gray>Staff</gray>]</dark_gray> <gray>{by} a réduit {player} au silence ({time}). {reason}"
            }
            Unmuted => "<green><white>{player}</white> peut de nouveau écrire.",
            NotMuted => "<red>{player} n'est pas réduit au silence.",
            YouWereUnmuted => "<green>Vous pouvez de nouveau écrire.",
            UnknownPlayer => "<red>Joueur inconnu : {player}",
            CannotMute => "<red>{player} ne peut pas être réduit au silence.",
            MuteListHeader => "<gold>Joueurs réduits au silence ({count}) :",
            MuteListEntry => {
                "<dark_gray>-</dark_gray> <white>{player}</white> <dark_gray>({time})</dark_gray> <gray>{reason}"
            }
            MuteListEmpty => "<gray>Personne n'est réduit au silence.",
            Permanent => "définitif",
            Mentioned => "<yellow>{player} vous a mentionné",
            Reloaded => "<green>Configuration de chat-format rechargée.",
            ReloadFailed => "<red>Échec du rechargement : {error}",
            CommandBlockedMuted => {
                "<red>Vous ne pouvez pas utiliser cette commande en étant réduit au silence."
            }
        },
    }
}

/// A message rendered with values (escaped here: pass raw player text).
pub fn tr(lang: Lang, msg: Msg, vars: &[(&str, &str)]) -> Text {
    let escaped: Vec<(&str, String)> = vars
        .iter()
        .map(|(k, v)| (*k, rustic::text::escape_mini(v)))
        .collect();
    let refs: Vec<(&str, &str)> = escaped.iter().map(|(k, v)| (*k, v.as_str())).collect();
    Text::mini(&fill(template(lang, msg), &refs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Msg; 25] = [
        Msg::MutedPerm,
        Msg::MutedTemp,
        Msg::TooFast,
        Msg::Duplicate,
        Msg::Caps,
        Msg::Link,
        Msg::Word,
        Msg::MuteDonePerm,
        Msg::MuteDoneTemp,
        Msg::YouWereMutedPerm,
        Msg::YouWereMutedTemp,
        Msg::MuteNotify,
        Msg::Unmuted,
        Msg::NotMuted,
        Msg::YouWereUnmuted,
        Msg::UnknownPlayer,
        Msg::CannotMute,
        Msg::MuteListHeader,
        Msg::MuteListEntry,
        Msg::MuteListEmpty,
        Msg::Permanent,
        Msg::Mentioned,
        Msg::Reloaded,
        Msg::ReloadFailed,
        Msg::CommandBlockedMuted,
    ];

    #[test]
    fn locales() {
        assert_eq!(Lang::from_code("fr_ca"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("FR"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("en_us"), Some(Lang::En));
        assert_eq!(Lang::from_code("de_de"), None);
    }

    #[test]
    fn both_languages_have_the_same_placeholders() {
        fn holes(s: &str) -> Vec<String> {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|(k, _)| k.to_owned()))
                .collect();
            v.sort();
            v
        }
        for m in ALL {
            let (en, fr) = (template(Lang::En, m), template(Lang::Fr, m));
            assert_eq!(holes(en), holes(fr), "{m:?}");
            if m != Msg::MuteListEntry {
                assert_ne!(en, fr, "{m:?} is not translated");
            }
            // Every template parses to some visible text.
            assert!(!Text::mini(en).plain().is_empty());
        }
    }

    #[test]
    fn values_are_escaped() {
        let t = tr(Lang::En, Msg::UnknownPlayer, &[("player", "<red>x")]);
        assert_eq!(t.plain(), "Unknown player: <red>x");
    }
}
