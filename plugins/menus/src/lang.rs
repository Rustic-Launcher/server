//! Fixed UI strings in English and French (MiniMessage-like templates with `{placeholders}`).

use rustic::Text;
use rustic::text::escape_mini;

/// A language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English.
    En,
    /// French.
    Fr,
}

impl Lang {
    /// From a config value or client locale (`fr_fr` -> French).
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

macro_rules! messages {
    ($($key:ident => $en:expr, $fr:expr;)*) => {
        /// Message keys.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[allow(missing_docs)]
        pub enum Msg { $($key),* }

        impl Msg {
            /// Every key (tests).
            #[cfg(test)]
            pub const ALL: &'static [Msg] = &[$(Msg::$key),*];
        }

        /// The template of a message.
        pub fn template(lang: Lang, msg: Msg) -> &'static str {
            match (lang, msg) {
                $((Lang::En, Msg::$key) => $en, (Lang::Fr, Msg::$key) => $fr,)*
            }
        }
    };
}

messages! {
    MainTitle => "<dark_gray>Main menu <gray>· {online} online", "<dark_gray>Menu principal <gray>· {online} en ligne";
    InfoName => "<aqua>Server information", "<aqua>Informations du serveur";
    InfoVersion => "<gray>Version: <white>{version}", "<gray>Version : <white>{version}";
    InfoPlayers => "<gray>Players: <white>{online}</white>/<white>{max}", "<gray>Joueurs : <white>{online}</white>/<white>{max}";
    InfoTps => "<gray>TPS: <white>{tps}</white> <dark_gray>({mspt} ms/tick)", "<gray>TPS : <white>{tps}</white> <dark_gray>({mspt} ms/tick)";
    InfoPlugins => "<gray>Plugins: <white>{count}", "<gray>Plugins : <white>{count}";
    InfoHint => "<yellow>Left-click: details in chat", "<yellow>Clic gauche : détails dans le chat";
    WarpsName => "<light_purple>Warps", "<light_purple>Téléportations";
    WarpsLore => "<gray>Travel around the server.", "<gray>Voyagez sur le serveur.";
    WarpsTitle => "<dark_gray>Warps", "<dark_gray>Téléportations";
    SettingsName => "<green>Settings", "<green>Paramètres";
    SettingsLore => "<gray>Your personal preferences.", "<gray>Vos préférences personnelles.";
    SettingsTitle => "<dark_gray>Settings", "<dark_gray>Paramètres";
    PlayersName => "<gold>Players online", "<gold>Joueurs en ligne";
    PlayersLore => "<gray>{online} players connected.", "<gray>{online} joueurs connectés.";
    PlayersTitle => "<dark_gray>Players <gray>({online}) · {page}/{pages}", "<dark_gray>Joueurs <gray>({online}) · {page}/{pages}";
    OpenHint => "<yellow>Click to open", "<yellow>Cliquez pour ouvrir";
    Back => "<yellow>Back", "<yellow>Retour";
    Close => "<red>Close", "<red>Fermer";
    PrevPage => "<yellow>Previous page", "<yellow>Page précédente";
    NextPage => "<yellow>Next page", "<yellow>Page suivante";
    PageInfo => "<white>Page {page}/{pages}", "<white>Page {page}/{pages}";
    On => "<green>Enabled", "<green>Activé";
    Off => "<red>Disabled", "<red>Désactivé";
    State => "<gray>State: {state}", "<gray>État : {state}";
    ToggleHints => "<yellow>Left-click: toggle <dark_gray>·</dark_gray> Right-click: details <dark_gray>·</dark_gray> Shift-click: default", "<yellow>Clic gauche : basculer <dark_gray>·</dark_gray> Clic droit : détails <dark_gray>·</dark_gray> Maj-clic : défaut";
    Toggled => "<gray>{name}: {state}", "<gray>{name} : {state}";
    ResetAllName => "<red>Reset all settings", "<red>Tout réinitialiser";
    ResetAllLore => "<gray>Back to the server defaults.", "<gray>Revenir aux valeurs par défaut.";
    ResetDone => "<green>Your settings were reset.", "<green>Vos paramètres ont été réinitialisés.";
    HeadLore1 => "<gray>World: <white>{world}", "<gray>Monde : <white>{world}";
    HeadLore2 => "<gray>Ping: <white>{ping} ms", "<gray>Latence : <white>{ping} ms";
    HeadHints => "<yellow>Left: info <dark_gray>·</dark_gray> Right: message <dark_gray>·</dark_gray> Shift: teleport", "<yellow>Gauche : infos <dark_gray>·</dark_gray> Droit : message <dark_gray>·</dark_gray> Maj : téléporter";
    PlayerInfo => "<gold>{name}</gold> <gray>is in <white>{world}</white> at <white>{x} {y} {z}</white>, ping <white>{ping} ms", "<gold>{name}</gold> <gray>est dans <white>{world}</white> en <white>{x} {y} {z}</white>, latence <white>{ping} ms";
    MessageLink => "<aqua>[Click to message {name}]", "<aqua>[Cliquez pour écrire à {name}]";
    PlayerGone => "<red>{name} is no longer online.", "<red>{name} n'est plus en ligne.";
    ThatsYou => "<gray>That is you!", "<gray>C'est vous !";
    NoTeleport => "<red>You may not teleport to players.", "<red>Vous ne pouvez pas vous téléporter aux joueurs.";
    TeleportQuestion => "<white>Teleport to <gold>{name}</gold>?", "<white>Se téléporter à <gold>{name}</gold> ?";
    Teleported => "<green>Teleported to {name}.", "<green>Téléporté à {name}.";
    ConfirmTitle => "<dark_gray>Are you sure?", "<dark_gray>Êtes-vous sûr ?";
    Yes => "<green><bold>Yes", "<green><bold>Oui";
    No => "<red><bold>No", "<red><bold>Non";
    Cancelled => "<gray>Cancelled.", "<gray>Annulé.";
    WarpHint => "<yellow>Left-click: go <dark_gray>·</dark_gray> Right-click: details", "<yellow>Clic gauche : y aller <dark_gray>·</dark_gray> Clic droit : détails";
    WarpQuestion => "<white>Go to <light_purple>{name}</light_purple>?", "<white>Aller à <light_purple>{name}</light_purple> ?";
    WarpDone => "<green>Warped to {name}.", "<green>Téléporté vers {name}.";
    WarpDetails => "<light_purple>{name}</light_purple><gray>: {description}", "<light_purple>{name}</light_purple><gray> : {description}";
    NoWarps => "<gray>No warp is configured.", "<gray>Aucune téléportation n'est configurée.";
    ServerInfoChat => "<aqua>{version}</aqua> <gray>· {online}/{max} players · {tps} TPS · {plugins} plugins", "<aqua>{version}</aqua> <gray>· {online}/{max} joueurs · {tps} TPS · {plugins} plugins";
}

/// A message rendered with values (escaped here: pass raw text).
pub fn tr(lang: Lang, msg: Msg, vars: &[(&str, &str)]) -> Text {
    Text::mini(&tr_mini(lang, msg, vars))
}

/// The template filled with escaped values, still as MiniMessage-like text.
pub fn tr_mini(lang: Lang, msg: Msg, vars: &[(&str, &str)]) -> String {
    let mut out = template(lang, msg).to_owned();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), &escape_mini(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_parse_and_share_placeholders() {
        fn holes(s: &str) -> Vec<String> {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|(k, _)| k.to_owned()))
                .collect();
            v.sort();
            v
        }
        for m in Msg::ALL {
            let (en, fr) = (template(Lang::En, *m), template(Lang::Fr, *m));
            assert_eq!(holes(en), holes(fr), "{m:?}");
            assert!(!Text::mini(en).plain().is_empty(), "{m:?}");
            assert!(!Text::mini(fr).plain().is_empty(), "{m:?}");
        }
        assert_eq!(Lang::from_code("fr_CA"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("xx"), None);
    }

    #[test]
    fn values_are_escaped() {
        assert_eq!(
            tr(Lang::En, Msg::PlayerGone, &[("name", "<b>x")]).plain(),
            "<b>x is no longer online."
        );
    }
}
