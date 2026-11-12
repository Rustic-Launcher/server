//! Player-facing messages in English and French (MiniMessage-like templates with `{placeholders}`).

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
    Help => "<gold>Mini-game</gold> <gray>· /mg join [arena] · /mg leave · /mg list · /mg stats [player] · /mg top [wins|games|points]", "<gold>Mini-jeu</gold> <gray>· /mg join [arène] · /mg leave · /mg list · /mg stats [joueur] · /mg top [wins|games|points]";
    HelpAdmin => "<gray>Admin: /mg create (arena) [spleef|koth] · setspawn · setlobby · setarea (arena) [radius] · start · stop · delete", "<gray>Admin : /mg create (arène) [spleef|koth] · setspawn · setlobby · setarea (arène) [rayon] · start · stop · delete";
    ModeSpleef => "Spleef", "Spleef";
    ModeKoth => "King of the Hill", "Roi de la colline";
    Joined => "<green>You joined <white>{arena}</white> ({count}/{max}).", "<green>Vous avez rejoint <white>{arena}</white> ({count}/{max}).";
    JoinedOther => "<gray>{player} joined ({count}/{max}).", "<gray>{player} a rejoint la partie ({count}/{max}).";
    Left => "<yellow>You left {arena}.", "<yellow>Vous avez quitté {arena}.";
    LeftOther => "<gray>{player} left ({count}/{max}).", "<gray>{player} est parti ({count}/{max}).";
    NotInGame => "<red>You are not in a game.", "<red>Vous n'êtes dans aucune partie.";
    AlreadyIn => "<red>You are already in {arena}: /mg leave first.", "<red>Vous êtes déjà dans {arena} : faites d'abord /mg leave.";
    Full => "<red>{arena} is full.", "<red>{arena} est complète.";
    InProgress => "<red>A game is being played in {arena}; try again soon.", "<red>Une partie est en cours dans {arena} ; réessayez bientôt.";
    NoArena => "<red>Unknown arena: {arena}", "<red>Arène inconnue : {arena}";
    NoOpenArena => "<red>No arena is open right now.", "<red>Aucune arène n'est ouverte pour le moment.";
    BadName => "<red>Arena names use a-z, 0-9 and _ (16 characters at most).", "<red>Les noms d'arène utilisent a-z, 0-9 et _ (16 caractères au plus).";
    BadMode => "<red>Modes: spleef, koth.", "<red>Modes : spleef, koth.";
    Exists => "<red>The arena {arena} already exists.", "<red>L'arène {arena} existe déjà.";
    Created => "<green>Arena <white>{arena}</white> ({mode}) created here, radius {radius}. Start point and lobby: here (/mg setspawn, /mg setlobby).", "<green>Arène <white>{arena}</white> ({mode}) créée ici, rayon {radius}. Départ et salle d'attente : ici (/mg setspawn, /mg setlobby).";
    SpawnSet => "<green>Start point of {arena} set here.", "<green>Point de départ de {arena} défini ici.";
    LobbySet => "<green>Lobby of {arena} set here.", "<green>Salle d'attente de {arena} définie ici.";
    AreaSet => "<green>Area of {arena}: radius {radius} around the block under you.", "<green>Zone de {arena} : rayon {radius} autour du bloc sous vous.";
    Deleted => "<green>Arena {arena} deleted.", "<green>Arène {arena} supprimée.";
    Busy => "<red>{arena} has a game in progress.", "<red>{arena} a une partie en cours.";
    ListHeader => "<gold>Arenas:", "<gold>Arènes :";
    ListEntry => "<dark_gray>-</dark_gray> <white>{arena}</white> <gray>{mode} · {state} · {count}/{max}", "<dark_gray>-</dark_gray> <white>{arena}</white> <gray>{mode} · {state} · {count}/{max}";
    ListEmpty => "<gray>No arena yet.", "<gray>Aucune arène pour l'instant.";
    ClickToJoin => "<yellow>Click to join", "<yellow>Cliquez pour rejoindre";
    StateWaiting => "waiting", "en attente";
    StateCountdown => "starting", "démarrage";
    StateRunning => "playing", "en jeu";
    StateEnding => "ending", "fin";
    BarWaiting => "<yellow>Waiting for players <white>{count}/{min}", "<yellow>En attente de joueurs <white>{count}/{min}";
    BarCountdown => "<green>Starting in <white>{seconds} s", "<green>Début dans <white>{seconds} s";
    BarRunning => "<aqua>{mode} <gray>· <white>{time}</white> left", "<aqua>{mode} <gray>· <white>{time}</white> restantes";
    BarEnding => "<gold>Winner: <white>{winner}", "<gold>Vainqueur : <white>{winner}";
    BarDraw => "<gray>Draw", "<gray>Égalité";
    CountdownSub => "<gray>Get ready!", "<gray>Préparez-vous !";
    Aborted => "<red>Not enough players: the countdown stopped.", "<red>Pas assez de joueurs : le compte à rebours est annulé.";
    Go => "<green>Go!", "<green>Partez !";
    GoSpleef => "<gray>Dig the snow under the others!", "<gray>Creusez la neige sous les autres !";
    GoKoth => "<gray>Hold the zone alone to score!", "<gray>Tenez la zone seul pour marquer !";
    Fell => "<red>{player} fell! <gray>{alive} still standing.", "<red>{player} est tombé ! <gray>{alive} encore debout.";
    YouFell => "<red>You fell!", "<red>Vous êtes tombé !";
    YouFellSub => "<gray>Wait for the end in the lobby.", "<gray>Attendez la fin dans la salle d'attente.";
    Victory => "<gold>Victory!", "<gold>Victoire !";
    GameOver => "<red>Game over", "<red>Partie terminée";
    WinnerSub => "<gray>Winner: <white>{winner}", "<gray>Vainqueur : <white>{winner}";
    DrawSub => "<gray>Draw!", "<gray>Égalité !";
    Won => "<gold>{winner} won {arena}!", "<gold>{winner} a gagné {arena} !";
    Draw => "<gray>{arena} ended in a draw.", "<gray>{arena} s'est terminée sur une égalité.";
    Reward => "<green>Reward: <white>{amount}", "<green>Récompense : <white>{amount}";
    King => "<gold>King: <white>{player}</white> <gray>({points}/{target})", "<gold>Roi : <white>{player}</white> <gray>({points}/{target})";
    Contested => "<red>Contested!", "<red>Zone disputée !";
    HillFree => "<gray>The hill is free: take it!", "<gray>La colline est libre : prenez-la !";
    Stopped => "<red>The game in {arena} was stopped.", "<red>La partie dans {arena} a été arrêtée.";
    Starting => "<green>Starting {arena}.", "<green>Démarrage de {arena}.";
    CannotStart => "<red>{arena} has nobody waiting or is already playing.", "<red>{arena} n'a personne en attente ou est déjà en jeu.";
    NothingToStop => "<red>No game is being played in {arena}.", "<red>Aucune partie n'est en cours dans {arena}.";
    StatsHeader => "<gold>Stats of {player}:", "<gold>Statistiques de {player} :";
    StatsLine => "<gray>Wins <white>{wins}</white> · games <white>{games}</white> · win rate <white>{rate}%</white> · points <white>{points}</white>", "<gray>Victoires <white>{wins}</white> · parties <white>{games}</white> · taux <white>{rate} %</white> · points <white>{points}</white>";
    NoStats => "<gray>{player} has not played yet.", "<gray>{player} n'a pas encore joué.";
    UnknownPlayer => "<red>Unknown player: {player}", "<red>Joueur inconnu : {player}";
    TopHeader => "<gold>Best players ({stat}):", "<gold>Meilleurs joueurs ({stat}) :";
    TopLine => "<yellow>#{rank}</yellow> <white>{player}</white> <gray>{wins} wins · {games} games · {points} points", "<yellow>#{rank}</yellow> <white>{player}</white> <gray>{wins} victoires · {games} parties · {points} points";
    TopHover => "<gray>Click for the stats of {player}", "<gray>Cliquez pour les statistiques de {player}";
    TopEmpty => "<gray>Nobody has played yet.", "<gray>Personne n'a encore joué.";
    SidebarTitle => "<gold><bold>{mode}</bold> <gray>{arena}", "<gold><bold>{mode}</bold> <gray>{arena}";
    SidebarTime => "Time left", "Temps restant";
    ShutDown => "<red>The mini-game was stopped by the server.", "<red>Le mini-jeu a été arrêté par le serveur.";
    ToolName => "<aqua>Spleef shovel", "<aqua>Pelle de spleef";
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
            tr(Lang::En, Msg::NoArena, &[("arena", "<b>x")]).plain(),
            "Unknown arena: <b>x"
        );
    }
}
