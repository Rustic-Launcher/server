//! Player-facing messages in English and French (`lang = "en" | "fr"` in `config.toml`).
//!
//! Templates are MiniMessage text with `{name}` placeholders; values are escaped before they
//! are spliced in, so player names or nodes can never inject tags. Every message gets the same
//! `[Perms]` header and palette: gray text, yellow values, red errors, green confirmations.

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

/// The header every message starts with.
pub const HEADER: &str = "<dark_gray>[<gold>Perms</gold>]</dark_gray> ";

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
    // Errors
    UnknownUser => "<red>Unknown player <yellow>{name}</yellow>: they never joined this server.",
        "<red>Joueur inconnu <yellow>{name}</yellow> : il n'est jamais venu sur ce serveur.";
    UnknownGroup => "<red>The group <yellow>{group}</yellow> does not exist.",
        "<red>Le groupe <yellow>{group}</yellow> n'existe pas.";
    UnknownTrack => "<red>The track <yellow>{track}</yellow> does not exist.",
        "<red>La piste <yellow>{track}</yellow> n'existe pas.";
    GroupExists => "<red>The group <yellow>{group}</yellow> already exists.",
        "<red>Le groupe <yellow>{group}</yellow> existe déjà.";
    TrackExists => "<red>The track <yellow>{track}</yellow> already exists.",
        "<red>La piste <yellow>{track}</yellow> existe déjà.";
    InvalidNode => "<red>Invalid permission node <yellow>{node}</yellow> (segments of a-z 0-9 _ : + -, optional final <yellow>*</yellow>).",
        "<red>Permission invalide <yellow>{node}</yellow> (segments a-z 0-9 _ : + -, <yellow>*</yellow> final facultatif).";
    InvalidName => "<red>Invalid name <yellow>{name}</yellow> (1 to 32 characters among a-z 0-9 _ -).",
        "<red>Nom invalide <yellow>{name}</yellow> (1 à 32 caractères parmi a-z 0-9 _ -).";
    Cycle => "<red><yellow>{parent}</yellow> already inherits <yellow>{group}</yellow>: that would make a loop.",
        "<red><yellow>{parent}</yellow> hérite déjà de <yellow>{group}</yellow> : cela créerait une boucle.";
    DefaultGroup => "<red>The default group <yellow>{group}</yellow> cannot be deleted.",
        "<red>Le groupe par défaut <yellow>{group}</yellow> ne peut pas être supprimé.";
    AlreadyOnTrack => "<red><yellow>{group}</yellow> is already on track <yellow>{track}</yellow>.",
        "<red><yellow>{group}</yellow> est déjà sur la piste <yellow>{track}</yellow>.";
    NotOnTrack => "<red><yellow>{group}</yellow> is not on track <yellow>{track}</yellow>.",
        "<red><yellow>{group}</yellow> n'est pas sur la piste <yellow>{track}</yellow>.";
    AmbiguousTrack => "<red>This player is in several groups of track <yellow>{track}</yellow>; fix it by hand first.",
        "<red>Ce joueur est dans plusieurs groupes de la piste <yellow>{track}</yellow> ; corrigez d'abord à la main.";
    EmptyTrack => "<red>The track <yellow>{track}</yellow> has no group.",
        "<red>La piste <yellow>{track}</yellow> n'a aucun groupe.";
    FileError => "<red>File <yellow>{file}</yellow>: {error}",
        "<red>Fichier <yellow>{file}</yellow> : {error}";

    // Users
    UserHeader => "<gray>Player <yellow>{name}</yellow> <dark_gray>({uuid})</dark_gray>",
        "<gray>Joueur <yellow>{name}</yellow> <dark_gray>({uuid})</dark_gray>";
    PrimaryGroup => "<gray>Primary group: <yellow>{group}</yellow>",
        "<gray>Groupe principal : <yellow>{group}</yellow>";
    GroupsLine => "<gray>Groups: {groups}",
        "<gray>Groupes : {groups}";
    NodesLine => "<gray>Permissions ({count}):",
        "<gray>Permissions ({count}) :";
    PrefixLine => "<gray>Prefix: ",
        "<gray>Préfixe : ";
    SuffixLine => "<gray>Suffix: ",
        "<gray>Suffixe : ";
    ParentsLine => "<gray>Parents: {parents}",
        "<gray>Parents : {parents}";
    WeightLine => "<gray>Weight: <yellow>{weight}</yellow>",
        "<gray>Poids : <yellow>{weight}</yellow>";
    MembersLine => "<gray>Known members: <yellow>{count}</yellow>",
        "<gray>Membres connus : <yellow>{count}</yellow>";
    None => "none", "aucun";
    ExpiresIn => "expires in {time}", "expire dans {time}";
    UserGroupAdded => "<green>Added <yellow>{name}</yellow> to group <yellow>{group}</yellow>.",
        "<green><yellow>{name}</yellow> a été ajouté au groupe <yellow>{group}</yellow>.";
    UserGroupAddedTemp => "<green>Added <yellow>{name}</yellow> to group <yellow>{group}</yellow> for <yellow>{time}</yellow>.",
        "<green><yellow>{name}</yellow> a été ajouté au groupe <yellow>{group}</yellow> pour <yellow>{time}</yellow>.";
    UserGroupAlready => "<gray><yellow>{name}</yellow> is already in group <yellow>{group}</yellow>.",
        "<gray><yellow>{name}</yellow> est déjà dans le groupe <yellow>{group}</yellow>.";
    UserGroupRemoved => "<green>Removed <yellow>{name}</yellow> from group <yellow>{group}</yellow>.",
        "<green><yellow>{name}</yellow> a été retiré du groupe <yellow>{group}</yellow>.";
    UserGroupNotMember => "<gray><yellow>{name}</yellow> is not in group <yellow>{group}</yellow>.",
        "<gray><yellow>{name}</yellow> n'est pas dans le groupe <yellow>{group}</yellow>.";
    UserGroupSet => "<green><yellow>{name}</yellow> is now only in group <yellow>{group}</yellow>.",
        "<green><yellow>{name}</yellow> est maintenant uniquement dans le groupe <yellow>{group}</yellow>.";
    PermSet => "<green>Set <yellow>{node}</yellow> to <yellow>{value}</yellow> for {target}.",
        "<green><yellow>{node}</yellow> vaut maintenant <yellow>{value}</yellow> pour {target}.";
    PermSetTemp => "<green>Set <yellow>{node}</yellow> to <yellow>{value}</yellow> for {target} for <yellow>{time}</yellow>.",
        "<green><yellow>{node}</yellow> vaut <yellow>{value}</yellow> pour {target} pendant <yellow>{time}</yellow>.";
    PermUnset => "<green>Unset <yellow>{node}</yellow> for {target}.",
        "<green><yellow>{node}</yellow> a été retiré pour {target}.";
    PermNotSet => "<gray><yellow>{node}</yellow> is not set for {target}.",
        "<gray><yellow>{node}</yellow> n'est pas défini pour {target}.";
    TargetUser => "<yellow>{name}</yellow>", "<yellow>{name}</yellow>";
    TargetGroup => "group <yellow>{group}</yellow>", "le groupe <yellow>{group}</yellow>";
    Promoted => "<green>Promoted <yellow>{name}</yellow> from <yellow>{from}</yellow> to <yellow>{to}</yellow> on <yellow>{track}</yellow>.",
        "<green><yellow>{name}</yellow> a été promu de <yellow>{from}</yellow> à <yellow>{to}</yellow> sur <yellow>{track}</yellow>.";
    PromotedJoined => "<green><yellow>{name}</yellow> joins track <yellow>{track}</yellow> at <yellow>{group}</yellow>.",
        "<green><yellow>{name}</yellow> rejoint la piste <yellow>{track}</yellow> au groupe <yellow>{group}</yellow>.";
    PromoteTop => "<gray><yellow>{name}</yellow> is already at the top of <yellow>{track}</yellow>.",
        "<gray><yellow>{name}</yellow> est déjà au sommet de <yellow>{track}</yellow>.";
    Demoted => "<green>Demoted <yellow>{name}</yellow> from <yellow>{from}</yellow> to <yellow>{to}</yellow> on <yellow>{track}</yellow>.",
        "<green><yellow>{name}</yellow> a été rétrogradé de <yellow>{from}</yellow> à <yellow>{to}</yellow> sur <yellow>{track}</yellow>.";
    DemotedLeft => "<green><yellow>{name}</yellow> left track <yellow>{track}</yellow> (removed from <yellow>{group}</yellow>).",
        "<green><yellow>{name}</yellow> quitte la piste <yellow>{track}</yellow> (retiré de <yellow>{group}</yellow>).";
    DemoteNotOn => "<gray><yellow>{name}</yellow> is not on track <yellow>{track}</yellow>.",
        "<gray><yellow>{name}</yellow> n'est pas sur la piste <yellow>{track}</yellow>.";
    YourGroupsChanged => "<gray>Your rank is now <yellow>{group}</yellow>.",
        "<gray>Votre rang est maintenant <yellow>{group}</yellow>.";

    // Groups
    GroupHeader => "<gray>Group <yellow>{group}</yellow>",
        "<gray>Groupe <yellow>{group}</yellow>";
    GroupCreated => "<green>Created group <yellow>{group}</yellow> (weight <yellow>{weight}</yellow>).",
        "<green>Groupe <yellow>{group}</yellow> créé (poids <yellow>{weight}</yellow>).";
    GroupDeleted => "<green>Deleted group <yellow>{group}</yellow>.",
        "<green>Groupe <yellow>{group}</yellow> supprimé.";
    ParentAdded => "<green><yellow>{group}</yellow> now inherits <yellow>{parent}</yellow>.",
        "<green><yellow>{group}</yellow> hérite maintenant de <yellow>{parent}</yellow>.";
    ParentAlready => "<gray><yellow>{group}</yellow> already inherits <yellow>{parent}</yellow>.",
        "<gray><yellow>{group}</yellow> hérite déjà de <yellow>{parent}</yellow>.";
    ParentRemoved => "<green><yellow>{group}</yellow> no longer inherits <yellow>{parent}</yellow>.",
        "<green><yellow>{group}</yellow> n'hérite plus de <yellow>{parent}</yellow>.";
    ParentNotSet => "<gray><yellow>{group}</yellow> does not inherit <yellow>{parent}</yellow>.",
        "<gray><yellow>{group}</yellow> n'hérite pas de <yellow>{parent}</yellow>.";
    PrefixSet => "<green>Prefix of {target} set to: ",
        "<green>Préfixe de {target} défini : ";
    PrefixCleared => "<green>Prefix of {target} removed.",
        "<green>Préfixe de {target} supprimé.";
    SuffixSet => "<green>Suffix of {target} set to: ",
        "<green>Suffixe de {target} défini : ";
    SuffixCleared => "<green>Suffix of {target} removed.",
        "<green>Suffixe de {target} supprimé.";
    WeightSet => "<green>Weight of <yellow>{group}</yellow> set to <yellow>{weight}</yellow>.",
        "<green>Poids de <yellow>{group}</yellow> défini à <yellow>{weight}</yellow>.";
    GroupList => "<gray>Groups (<yellow>{count}</yellow>), click for details:",
        "<gray>Groupes (<yellow>{count}</yellow>), cliquez pour le détail :";

    // Tracks
    TrackCreated => "<green>Created track <yellow>{track}</yellow>.",
        "<green>Piste <yellow>{track}</yellow> créée.";
    TrackDeleted => "<green>Deleted track <yellow>{track}</yellow>.",
        "<green>Piste <yellow>{track}</yellow> supprimée.";
    TrackInfo => "<gray>Track <yellow>{track}</yellow>: {groups}",
        "<gray>Piste <yellow>{track}</yellow> : {groups}";
    TrackAppended => "<green>Appended <yellow>{group}</yellow> to track <yellow>{track}</yellow>.",
        "<green><yellow>{group}</yellow> ajouté à la fin de la piste <yellow>{track}</yellow>.";
    TrackInserted => "<green>Inserted <yellow>{group}</yellow> at position <yellow>{position}</yellow> of track <yellow>{track}</yellow>.",
        "<green><yellow>{group}</yellow> inséré en position <yellow>{position}</yellow> de la piste <yellow>{track}</yellow>.";
    TrackRemoved => "<green>Removed <yellow>{group}</yellow> from track <yellow>{track}</yellow>.",
        "<green><yellow>{group}</yellow> retiré de la piste <yellow>{track}</yellow>.";
    TrackList => "<gray>Tracks (<yellow>{count}</yellow>):",
        "<gray>Pistes (<yellow>{count}</yellow>) :";

    // Misc
    CheckResult => "<gray><yellow>{name}</yellow> → <yellow>{node}</yellow> = {value} <dark_gray>(from {source}, key {key})</dark_gray>",
        "<gray><yellow>{name}</yellow> → <yellow>{node}</yellow> = {value} <dark_gray>(par {source}, clé {key})</dark_gray>";
    CheckUndefined => "<gray><yellow>{name}</yellow> → <yellow>{node}</yellow> is <yellow>undefined</yellow> here: the node's default (or op status) applies.",
        "<gray><yellow>{name}</yellow> → <yellow>{node}</yellow> est <yellow>indéfini</yellow> ici : la valeur par défaut du nœud (ou le statut op) s'applique.";
    Reloaded => "<green>Reloaded: <yellow>{groups}</yellow> groups, <yellow>{users}</yellow> players, <yellow>{tracks}</yellow> tracks.",
        "<green>Rechargé : <yellow>{groups}</yellow> groupes, <yellow>{users}</yellow> joueurs, <yellow>{tracks}</yellow> pistes.";
    Exported => "<green>Exported to <yellow>{file}</yellow> (<yellow>{groups}</yellow> groups, <yellow>{users}</yellow> players, <yellow>{tracks}</yellow> tracks).",
        "<green>Exporté vers <yellow>{file}</yellow> (<yellow>{groups}</yellow> groupes, <yellow>{users}</yellow> joueurs, <yellow>{tracks}</yellow> pistes).";
    Imported => "<green>Imported <yellow>{file}</yellow>: <yellow>{groups}</yellow> groups, <yellow>{users}</yellow> players, <yellow>{tracks}</yellow> tracks.",
        "<green><yellow>{file}</yellow> importé : <yellow>{groups}</yellow> groupes, <yellow>{users}</yellow> joueurs, <yellow>{tracks}</yellow> pistes.";
    GroupExpired => "<gray>Your temporary group <yellow>{group}</yellow> has expired.",
        "<gray>Votre groupe temporaire <yellow>{group}</yellow> a expiré.";
    NodeExpired => "<gray>Your temporary permission <yellow>{node}</yellow> has expired.",
        "<gray>Votre permission temporaire <yellow>{node}</yellow> a expiré.";
    HelpHeader => "<gray>Commands (click one to fill it in):",
        "<gray>Commandes (cliquez pour la saisir) :";
}

/// Fills a template: every `{key}` is replaced with the escaped value.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), &escape_mini(v));
    }
    out
}

/// Fills a template whose values are already MiniMessage (not escaped).
pub fn fill_raw(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// A message with the `[Perms]` header.
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

/// A message without header (continuation lines).
pub fn line(lang: Lang, key: Msg, args: &[(&str, &str)]) -> Text {
    Text::mini(&fill(key.template(lang), args))
}

/// Renders stored chat meta: legacy `&`/`§` codes when present, MiniMessage otherwise.
pub fn render_meta(raw: &str) -> Text {
    if looks_legacy(raw) {
        Text::legacy(raw)
    } else {
        Text::mini(raw)
    }
}

/// Whether text uses legacy formatting codes (`&a`, `§l`, `&#RRGGBB`).
pub fn looks_legacy(raw: &str) -> bool {
    let chars: Vec<char> = raw.chars().collect();
    chars.windows(2).any(|w| {
        (w[0] == '&' || w[0] == '§')
            && (w[1].is_ascii_hexdigit() || "klmnorxKLMNORX#".contains(w[1]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_has_both_languages_and_same_placeholders() {
        for m in Msg::ALL {
            let en = m.template(Lang::En);
            let fr = m.template(Lang::Fr);
            assert!(!en.is_empty() && !fr.is_empty(), "{m:?}");
            let holes = |s: &str| {
                let mut v: Vec<String> = s
                    .split('{')
                    .skip(1)
                    .filter_map(|p| p.split_once('}').map(|(k, _)| k.to_owned()))
                    .collect();
                v.sort();
                v
            };
            assert_eq!(holes(en), holes(fr), "{m:?}");
        }
    }

    #[test]
    fn values_are_escaped() {
        let t = fill("<gray>{name}", &[("name", "<red>evil")]);
        assert_eq!(t, "<gray>\\<red>evil");
        let text = msg(Lang::En, Msg::UnknownGroup, &[("group", "<bold>x")]);
        assert!(text.plain().contains("<bold>x"));
        assert!(text.plain().starts_with("[Perms] "));
    }

    #[test]
    fn languages_parse_from_config() {
        #[derive(Deserialize)]
        struct C {
            lang: Lang,
        }
        let c: C = rustic::toml::from_str("lang = \"fr\"").unwrap();
        assert_eq!(c.lang, Lang::Fr);
        assert_eq!(
            msg(Lang::Fr, Msg::GroupDeleted, &[("group", "vip")]).plain(),
            "[Perms] Groupe vip supprimé."
        );
    }

    #[test]
    fn meta_rendering() {
        assert!(looks_legacy("&6[VIP] "));
        assert!(looks_legacy("§l!"));
        assert!(!looks_legacy("<gold>[VIP]"));
        assert!(!looks_legacy("Tom & Jerry"));
        assert_eq!(render_meta("&6[VIP]").plain(), "[VIP]");
        assert_eq!(render_meta("<gold>[VIP]</gold>").plain(), "[VIP]");
    }
}
