//! Argument parsers for every [`ArgKind`].
//!
//! A parser gets the remaining input (starting at an argument, leading spaces removed) and
//! returns the alternatives it can read: `(value, rest)` pairs, most greedy first (only `item`
//! has two: with and without a count). An alternative is valid only if `rest` is empty or
//! starts with whitespace.

use crate::model::{
    ArgKind, ArgSpec, ArgValue, BlockPos, CommandSender, ItemStack, Location, Uuid,
};

/// What parsers need from the game.
pub trait Resolver {
    /// Online player by exact name (case-insensitive).
    fn player_by_name(&mut self, name: &str) -> Option<Uuid>;
    /// Online players.
    fn online_players(&mut self) -> Vec<Uuid>;
    /// A player's name.
    fn player_name(&mut self, player: Uuid) -> Option<String>;
    /// Whether online.
    fn is_online(&mut self, player: Uuid) -> bool;
    /// A player's location.
    fn location(&mut self, player: Uuid) -> Option<Location>;
    /// Spawn of a world (for `@p` from the console).
    fn world_spawn(&mut self, world: &str) -> Option<Location>;
    /// Loaded worlds (`None` if unknown: any syntactically valid id is accepted).
    fn worlds(&mut self) -> Option<Vec<String>>;
    /// A random number in `0..n` (n > 0).
    fn random_below(&mut self, n: usize) -> usize;
}

/// A parse failure: how far it got (for picking the most relevant error) and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgError {
    /// Bytes of the whole input consumed before the error.
    pub position: usize,
    /// Message for the user.
    pub message: String,
}

/// Default world for console senders and relative coordinates without a location.
pub const DEFAULT_WORLD: &str = "minecraft:overworld";

/// Split the first whitespace-delimited token.
pub fn token(input: &str) -> (&str, &str) {
    match input.find(char::is_whitespace) {
        Some(i) => (&input[..i], &input[i..]),
        None => (input, ""),
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-')
}

fn bounds_f64(spec: &ArgSpec, v: f64) -> Result<(), String> {
    if let Some(min) = spec.min
        && v < min
    {
        return Err(format!("{} must be at least {min}, got {v}", spec.name));
    }
    if let Some(max) = spec.max
        && v > max
    {
        return Err(format!("{} must be at most {max}, got {v}", spec.name));
    }
    Ok(())
}

/// Parse a quoted string starting at `input[0]` (a `"` or `'`); returns (unescaped, rest).
pub fn quoted(input: &str) -> Result<(String, &str), String> {
    let mut chars = input.char_indices();
    let Some((_, q)) = chars.next() else {
        return Err("expected a string".into());
    };
    let mut out = String::new();
    let mut escaped = false;
    for (i, c) in chars {
        if escaped {
            match c {
                '\\' | '"' | '\'' => out.push(c),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => return Err(format!("invalid escape `\\{other}` in string")),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == q {
            return Ok((out, &input[i + c.len_utf8()..]));
        } else {
            out.push(c);
        }
    }
    Err("unterminated quoted string".into())
}

/// Parse a duration (`10s`, `5m`, `2h`, `20t`, `1d`, combinations like `1h30m`, or a bare
/// number of ticks) into ticks.
pub fn parse_duration(s: &str) -> Result<u64, String> {
    if s.is_empty() {
        return Err("expected a duration like 10s, 5m, 2h, 20t or 1d".into());
    }
    if s.bytes().all(|b| b.is_ascii_digit()) {
        return s
            .parse::<u64>()
            .map_err(|_| format!("duration `{s}` is too large"));
    }
    let mut total: u64 = 0;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let mult: u64 = match c.to_ascii_lowercase() {
            't' => 1,
            's' => 20,
            'm' => 1_200,
            'h' => 72_000,
            'd' => 1_728_000,
            _ => {
                return Err(format!(
                    "invalid duration `{s}`: unknown unit `{c}` (use t, s, m, h, d)"
                ));
            }
        };
        if num.is_empty() {
            return Err(format!(
                "invalid duration `{s}`: a number must come before `{c}`"
            ));
        }
        let n: u64 = num
            .parse()
            .map_err(|_| format!("duration `{s}` is too large"))?;
        total = n
            .checked_mul(mult)
            .and_then(|v| total.checked_add(v))
            .ok_or_else(|| format!("duration `{s}` is too large"))?;
        num.clear();
    }
    if !num.is_empty() {
        return Err(format!(
            "invalid duration `{s}`: missing unit after {num} (use t, s, m, h, d)"
        ));
    }
    Ok(total)
}

/// Normalise an item id: `diamond` → `minecraft:diamond`; validated lowercase.
pub fn parse_item_id(s: &str) -> Result<String, String> {
    let full = if s.contains(':') {
        s.to_owned()
    } else {
        format!("minecraft:{s}")
    };
    let (ns, path) = full.split_once(':').unwrap_or(("", ""));
    let ok_ns = !ns.is_empty()
        && ns
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'));
    let ok_path = !path.is_empty()
        && path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'));
    if ok_ns && ok_path {
        Ok(full)
    } else {
        Err(format!("`{s}` is not an item id (namespace:id, lowercase)"))
    }
}

/// Normalise a world id: `overworld`, `the_nether`, `the_end` and bare names get the
/// `minecraft:` namespace.
pub fn parse_world_id(s: &str) -> Result<String, String> {
    parse_item_id(s).map_err(|_| format!("`{s}` is not a world id (namespace:path)"))
}

fn selector_players(
    sel: &str,
    sender: &CommandSender,
    r: &mut dyn Resolver,
) -> Result<Vec<Uuid>, String> {
    if sel.contains('[') {
        return Err(format!("selector arguments are not supported: `{sel}`"));
    }
    match sel {
        "@a" => Ok(r.online_players()),
        "@s" => match sender {
            CommandSender::Player(u) => Ok(vec![*u]),
            CommandSender::Console => Err("@s needs a player sender".into()),
        },
        "@r" => {
            let all = r.online_players();
            if all.is_empty() {
                Ok(vec![])
            } else {
                let i = r.random_below(all.len());
                Ok(vec![all[i]])
            }
        }
        "@p" => {
            let origin = match sender {
                CommandSender::Player(u) => r.location(*u),
                CommandSender::Console => None,
            }
            .or_else(|| r.world_spawn(DEFAULT_WORLD))
            .unwrap_or(Location {
                world: DEFAULT_WORLD.into(),
                ..Location::default()
            });
            let mut best: Option<(f64, Uuid)> = None;
            for p in r.online_players() {
                if let Some(l) = r.location(p)
                    && l.world == origin.world
                {
                    let d = (l.x - origin.x).powi(2)
                        + (l.y - origin.y).powi(2)
                        + (l.z - origin.z).powi(2);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, p));
                    }
                }
            }
            Ok(best.map(|(_, p)| vec![p]).unwrap_or_default())
        }
        "@e" => Err("@e selects entities; only players are allowed here".into()),
        other => Err(format!("unknown selector `{other}` (use @p, @r, @a or @s)")),
    }
}

fn one_player(
    tok: &str,
    sender: &CommandSender,
    r: &mut dyn Resolver,
) -> Result<Vec<Uuid>, String> {
    if tok.starts_with('@') {
        return selector_players(tok, sender, r);
    }
    if tok.len() == 36
        && let Ok(u) = tok.parse::<Uuid>()
    {
        return if r.is_online(u) {
            Ok(vec![u])
        } else {
            Err(format!("player {tok} is not online"))
        };
    }
    if tok.is_empty()
        || tok.len() > 16
        || !tok.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(format!("`{tok}` is not a player name"));
    }
    r.player_by_name(tok)
        .map(|u| vec![u])
        .ok_or_else(|| format!("player `{tok}` is not online"))
}

fn coord(tok: &str, base: Option<f64>, integer: bool) -> Result<f64, String> {
    if let Some(rest) = tok.strip_prefix('~') {
        let base = base.ok_or("relative coordinates (~) need a player sender")?;
        let off = if rest.is_empty() {
            0.0
        } else if integer {
            rest.parse::<i32>()
                .map_err(|_| format!("`{tok}` is not a relative block coordinate"))?
                as f64
        } else {
            rest.parse::<f64>()
                .map_err(|_| format!("`{tok}` is not a relative coordinate"))?
        };
        let v = base + off;
        return if v.is_finite() {
            Ok(v)
        } else {
            Err(format!("`{tok}` is out of range"))
        };
    }
    if tok.starts_with('^') {
        return Err("local coordinates (^) are not supported".into());
    }
    if integer {
        tok.parse::<i32>()
            .map(f64::from)
            .map_err(|_| format!("`{tok}` is not a block coordinate"))
    } else {
        match tok.parse::<f64>() {
            Ok(v) if v.is_finite() => Ok(v),
            _ => Err(format!("`{tok}` is not a coordinate")),
        }
    }
}

fn three(input: &str) -> Option<([&str; 3], &str)> {
    let (a, r) = token(input);
    let (b, r) = token(r.trim_start());
    let (c, r) = token(r.trim_start());
    if a.is_empty() || b.is_empty() || c.is_empty() {
        None
    } else {
        Some(([a, b, c], r))
    }
}

/// Parse one argument. `consumed` is how many bytes of the whole input precede `input` (for
/// error positions).
pub fn parse_arg<'i>(
    spec: &ArgSpec,
    input: &'i str,
    consumed: usize,
    sender: &CommandSender,
    r: &mut dyn Resolver,
) -> Result<Vec<(ArgValue, &'i str)>, ArgError> {
    let err = |message: String| ArgError {
        position: consumed,
        message,
    };
    if input.is_empty() {
        return Err(err(format!("missing argument <{}>", spec.name)));
    }
    let (tok, rest) = token(input);
    let one = |v: ArgValue| Ok(vec![(v, rest)]);
    match spec.kind {
        ArgKind::Word => {
            if tok.chars().all(is_word_char) {
                one(ArgValue::Word(tok.to_owned()))
            } else {
                Err(err(format!(
                    "`{tok}` is not a single word ([A-Za-z0-9_.+-])"
                )))
            }
        }
        ArgKind::String => {
            if input.starts_with('"') || input.starts_with('\'') {
                let (s, rest) = quoted(input).map_err(err)?;
                if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
                    return Err(err("expected a space after the closing quote".into()));
                }
                Ok(vec![(ArgValue::String(s), rest)])
            } else {
                one(ArgValue::String(tok.to_owned()))
            }
        }
        ArgKind::Greedy => {
            let text = input.trim_end();
            Ok(vec![(ArgValue::Greedy(text.to_owned()), "")])
        }
        ArgKind::Integer => {
            let v: i64 = tok.parse().map_err(|_| {
                err(format!(
                    "expected an integer for <{}>, got `{tok}`",
                    spec.name
                ))
            })?;
            bounds_f64(spec, v as f64).map_err(err)?;
            one(ArgValue::Integer(v))
        }
        ArgKind::Number => {
            let v: f64 = tok
                .parse()
                .ok()
                .filter(|v: &f64| v.is_finite())
                .ok_or_else(|| {
                    err(format!(
                        "expected a number for <{}>, got `{tok}`",
                        spec.name
                    ))
                })?;
            bounds_f64(spec, v).map_err(err)?;
            one(ArgValue::Number(v))
        }
        ArgKind::Boolean => match tok.to_ascii_lowercase().as_str() {
            "true" => one(ArgValue::Boolean(true)),
            "false" => one(ArgValue::Boolean(false)),
            _ => Err(err(format!(
                "expected true or false for <{}>, got `{tok}`",
                spec.name
            ))),
        },
        ArgKind::Player => {
            let list = one_player(tok, sender, r).map_err(err)?;
            match list.as_slice() {
                [u] => one(ArgValue::Player(*u)),
                [] => Err(err(format!("no player matches `{tok}`"))),
                _ => Err(err(format!(
                    "`{tok}` matches several players; one is expected"
                ))),
            }
        }
        ArgKind::Players => {
            let list = one_player(tok, sender, r).map_err(err)?;
            if list.is_empty() {
                Err(err(format!("no player matches `{tok}`")))
            } else {
                one(ArgValue::Players(list))
            }
        }
        ArgKind::Position | ArgKind::BlockPos => {
            let integer = spec.kind == ArgKind::BlockPos;
            let ([a, b, c], rest) = three(input)
                .ok_or_else(|| err(format!("<{}> needs three coordinates: x y z", spec.name)))?;
            let base = match sender {
                CommandSender::Player(u) if [a, b, c].iter().any(|t| t.starts_with('~')) => {
                    r.location(*u)
                }
                _ => None,
            };
            let (bx, by, bz) = match &base {
                Some(l) if integer => {
                    let p = l.block_pos();
                    (Some(p.x as f64), Some(p.y as f64), Some(p.z as f64))
                }
                Some(l) => (Some(l.x), Some(l.y), Some(l.z)),
                None => (None, None, None),
            };
            let x = coord(a, bx, integer).map_err(err)?;
            let y = coord(b, by, integer).map_err(err)?;
            let z = coord(c, bz, integer).map_err(err)?;
            if integer {
                let fits = |v: f64| v >= f64::from(i32::MIN) && v <= f64::from(i32::MAX);
                if !(fits(x) && fits(y) && fits(z)) {
                    return Err(err("block position out of range".into()));
                }
                Ok(vec![(
                    ArgValue::BlockPos(BlockPos {
                        x: x as i32,
                        y: y as i32,
                        z: z as i32,
                    }),
                    rest,
                )])
            } else {
                let (world, yaw, pitch) = match (&base, sender) {
                    (Some(l), _) => (l.world.clone(), l.yaw, l.pitch),
                    (None, CommandSender::Player(u)) => match r.location(*u) {
                        Some(l) => (l.world, l.yaw, l.pitch),
                        None => (DEFAULT_WORLD.to_owned(), 0.0, 0.0),
                    },
                    (None, CommandSender::Console) => (DEFAULT_WORLD.to_owned(), 0.0, 0.0),
                };
                Ok(vec![(
                    ArgValue::Position(Location {
                        world,
                        x,
                        y,
                        z,
                        yaw,
                        pitch,
                    }),
                    rest,
                )])
            }
        }
        ArgKind::Item => {
            let id = parse_item_id(tok).map_err(err)?;
            let mut out = Vec::new();
            let (count_tok, after) = token(rest.trim_start());
            if !count_tok.is_empty()
                && count_tok.bytes().all(|b| b.is_ascii_digit())
                && let Ok(n) = count_tok.parse::<u32>()
                && n >= 1
            {
                out.push((ArgValue::Item(ItemStack::new(id.clone(), n)), after));
            }
            out.push((ArgValue::Item(ItemStack::new(id, 1)), rest));
            Ok(out)
        }
        ArgKind::World => {
            let id = parse_world_id(tok).map_err(err)?;
            if let Some(worlds) = r.worlds()
                && !worlds.contains(&id)
            {
                return Err(err(format!("unknown world `{id}`")));
            }
            one(ArgValue::World(id))
        }
        ArgKind::Duration => {
            let ticks = parse_duration(tok).map_err(err)?;
            bounds_f64(spec, ticks as f64).map_err(err)?;
            one(ArgValue::Duration(ticks))
        }
    }
}

/// Completions for an argument of `spec` given what is typed (`partial`). Dynamic
/// suggestions are not included (the plugin is asked separately).
pub fn suggest(spec: &ArgSpec, partial: &str, r: &mut dyn Resolver) -> Vec<String> {
    let mut out: Vec<String> = spec.suggestions.clone();
    match spec.kind {
        ArgKind::Boolean => out.extend(["true".to_owned(), "false".to_owned()]),
        ArgKind::Player | ArgKind::Players => {
            let online = r.online_players();
            out.extend(online.into_iter().filter_map(|u| r.player_name(u)));
            if partial.starts_with('@') {
                out.extend(["@p", "@r", "@s"].map(str::to_owned));
                if spec.kind == ArgKind::Players {
                    out.push("@a".to_owned());
                }
            }
        }
        ArgKind::World => out.extend(r.worlds().unwrap_or_default()),
        ArgKind::Position | ArgKind::BlockPos if partial.is_empty() => out.push("~".to_owned()),
        _ => {}
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fixed set of players for tests.
    #[derive(Default)]
    pub struct FakeResolver {
        pub players: Vec<(Uuid, String, Location)>,
        pub worlds: Option<Vec<String>>,
        pub next_random: usize,
    }

    impl FakeResolver {
        pub fn with_players() -> Self {
            let loc = |x: f64| Location {
                world: DEFAULT_WORLD.into(),
                x,
                y: 64.0,
                z: 0.0,
                yaw: 90.0,
                pitch: 0.0,
            };
            Self {
                players: vec![
                    (Uuid::new(0, 1), "Alice".into(), loc(0.0)),
                    (Uuid::new(0, 2), "Bob".into(), loc(10.0)),
                    (Uuid::new(0, 3), "Carol".into(), loc(100.5)),
                ],
                worlds: Some(vec![DEFAULT_WORLD.into(), "minecraft:the_nether".into()]),
                next_random: 1,
            }
        }
    }

    impl Resolver for FakeResolver {
        fn player_by_name(&mut self, name: &str) -> Option<Uuid> {
            self.players
                .iter()
                .find(|p| p.1.eq_ignore_ascii_case(name))
                .map(|p| p.0)
        }
        fn online_players(&mut self) -> Vec<Uuid> {
            self.players.iter().map(|p| p.0).collect()
        }
        fn player_name(&mut self, player: Uuid) -> Option<String> {
            self.players
                .iter()
                .find(|p| p.0 == player)
                .map(|p| p.1.clone())
        }
        fn is_online(&mut self, player: Uuid) -> bool {
            self.players.iter().any(|p| p.0 == player)
        }
        fn location(&mut self, player: Uuid) -> Option<Location> {
            self.players
                .iter()
                .find(|p| p.0 == player)
                .map(|p| p.2.clone())
        }
        fn world_spawn(&mut self, _world: &str) -> Option<Location> {
            None
        }
        fn worlds(&mut self) -> Option<Vec<String>> {
            self.worlds.clone()
        }
        fn random_below(&mut self, n: usize) -> usize {
            self.next_random % n
        }
    }

    fn spec(kind: ArgKind) -> ArgSpec {
        ArgSpec::new("x", kind)
    }

    fn parse1(kind: ArgKind, input: &str, sender: CommandSender) -> Result<ArgValue, String> {
        let mut r = FakeResolver::with_players();
        parse_arg(&spec(kind), input, 0, &sender, &mut r)
            .map(|mut v| v.remove(0).0)
            .map_err(|e| e.message)
    }

    const ALICE: CommandSender = CommandSender::Player(Uuid { hi: 0, lo: 1 });
    const CONSOLE: CommandSender = CommandSender::Console;

    #[test]
    fn words_strings_greedy() {
        assert_eq!(
            parse1(ArgKind::Word, "ab_c.d+e-f rest", CONSOLE),
            Ok(ArgValue::Word("ab_c.d+e-f".into()))
        );
        assert!(parse1(ArgKind::Word, "a:b", CONSOLE).is_err());
        assert_eq!(
            parse1(ArgKind::String, "plain x", CONSOLE),
            Ok(ArgValue::String("plain".into()))
        );
        assert_eq!(
            parse1(ArgKind::String, r#""a \"b\" \\ c" x"#, CONSOLE),
            Ok(ArgValue::String(r#"a "b" \ c"#.into()))
        );
        assert_eq!(
            parse1(ArgKind::String, "'it''", CONSOLE).map_err(|_| ()),
            Err(())
        );
        assert_eq!(
            parse1(ArgKind::String, "'it\\'s'", CONSOLE),
            Ok(ArgValue::String("it's".into()))
        );
        assert!(
            parse1(ArgKind::String, "\"open", CONSOLE)
                .unwrap_err()
                .contains("unterminated")
        );
        assert!(
            parse1(ArgKind::String, "\"a\\q\"", CONSOLE)
                .unwrap_err()
                .contains("escape")
        );
        assert_eq!(
            parse1(ArgKind::Greedy, "hello  world  ", CONSOLE),
            Ok(ArgValue::Greedy("hello  world".into()))
        );
        assert!(
            parse1(ArgKind::Greedy, "", CONSOLE)
                .unwrap_err()
                .contains("missing")
        );
    }

    #[test]
    fn numbers_and_bounds() {
        let mut s = spec(ArgKind::Integer);
        s.min = Some(1.0);
        s.max = Some(10.0);
        let mut r = FakeResolver::default();
        assert!(parse_arg(&s, "5", 0, &CONSOLE, &mut r).is_ok());
        assert!(
            parse_arg(&s, "0", 0, &CONSOLE, &mut r)
                .unwrap_err()
                .message
                .contains("at least")
        );
        assert!(
            parse_arg(&s, "11", 0, &CONSOLE, &mut r)
                .unwrap_err()
                .message
                .contains("at most")
        );
        assert!(parse_arg(&s, "1.5", 0, &CONSOLE, &mut r).is_err());
        assert_eq!(
            parse1(ArgKind::Integer, "-42", CONSOLE),
            Ok(ArgValue::Integer(-42))
        );
        assert_eq!(
            parse1(ArgKind::Number, "2.5", CONSOLE),
            Ok(ArgValue::Number(2.5))
        );
        assert!(parse1(ArgKind::Number, "NaN", CONSOLE).is_err());
        assert!(parse1(ArgKind::Number, "inf", CONSOLE).is_err());
        assert_eq!(
            parse1(ArgKind::Boolean, "TRUE", CONSOLE),
            Ok(ArgValue::Boolean(true))
        );
        assert!(parse1(ArgKind::Boolean, "yes", CONSOLE).is_err());
    }

    #[test]
    fn players_and_selectors() {
        assert_eq!(
            parse1(ArgKind::Player, "bob", CONSOLE),
            Ok(ArgValue::Player(Uuid::new(0, 2)))
        );
        assert!(
            parse1(ArgKind::Player, "Dave", CONSOLE)
                .unwrap_err()
                .contains("not online")
        );
        assert!(parse1(ArgKind::Player, "bad-name", CONSOLE).is_err());
        assert_eq!(
            parse1(
                ArgKind::Player,
                "00000000-0000-0000-0000-000000000003",
                CONSOLE
            ),
            Ok(ArgValue::Player(Uuid::new(0, 3)))
        );
        assert_eq!(
            parse1(ArgKind::Player, "@s", ALICE),
            Ok(ArgValue::Player(Uuid::new(0, 1)))
        );
        assert!(parse1(ArgKind::Player, "@s", CONSOLE).is_err());
        assert!(
            parse1(ArgKind::Player, "@a", CONSOLE)
                .unwrap_err()
                .contains("several")
        );
        // @p from Carol (x=100.5) is Carol herself; from the console (origin 0,0,0) Alice.
        let carol = CommandSender::Player(Uuid::new(0, 3));
        assert_eq!(
            parse1(ArgKind::Player, "@p", carol),
            Ok(ArgValue::Player(Uuid::new(0, 3)))
        );
        assert_eq!(
            parse1(ArgKind::Player, "@p", CONSOLE),
            Ok(ArgValue::Player(Uuid::new(0, 1)))
        );
        assert_eq!(
            parse1(ArgKind::Player, "@r", CONSOLE),
            Ok(ArgValue::Player(Uuid::new(0, 2)))
        );
        assert_eq!(
            parse1(ArgKind::Players, "@a", CONSOLE),
            Ok(ArgValue::Players(vec![
                Uuid::new(0, 1),
                Uuid::new(0, 2),
                Uuid::new(0, 3)
            ]))
        );
        assert_eq!(
            parse1(ArgKind::Players, "Bob", CONSOLE),
            Ok(ArgValue::Players(vec![Uuid::new(0, 2)]))
        );
        assert!(
            parse1(ArgKind::Players, "@a[distance=..5]", CONSOLE)
                .unwrap_err()
                .contains("not supported")
        );
        assert!(parse1(ArgKind::Players, "@e", CONSOLE).is_err());
        let mut empty = FakeResolver::default();
        assert!(
            parse_arg(&spec(ArgKind::Players), "@a", 0, &CONSOLE, &mut empty)
                .unwrap_err()
                .message
                .contains("no player")
        );
    }

    #[test]
    fn positions() {
        let v = parse1(ArgKind::Position, "1 2.5 -3 rest", CONSOLE).unwrap();
        match v {
            ArgValue::Position(l) => {
                assert_eq!((l.x, l.y, l.z), (1.0, 2.5, -3.0));
                assert_eq!(l.world, DEFAULT_WORLD);
            }
            other => panic!("{other:?}"),
        }
        match parse1(ArgKind::Position, "~ ~1 ~-0.5", ALICE).unwrap() {
            ArgValue::Position(l) => {
                assert_eq!((l.x, l.y, l.z), (0.0, 65.0, -0.5));
                assert_eq!(l.yaw, 90.0);
            }
            other => panic!("{other:?}"),
        }
        assert!(
            parse1(ArgKind::Position, "~ 1 2", CONSOLE)
                .unwrap_err()
                .contains("player sender")
        );
        assert!(
            parse1(ArgKind::Position, "1 2", CONSOLE)
                .unwrap_err()
                .contains("three")
        );
        assert!(parse1(ArgKind::Position, "^ ^ ^", ALICE).is_err());
        let carol = CommandSender::Player(Uuid::new(0, 3));
        assert_eq!(
            parse1(ArgKind::BlockPos, "~1 ~ ~-1", carol),
            Ok(ArgValue::BlockPos(BlockPos {
                x: 101,
                y: 64,
                z: -1
            }))
        );
        assert!(parse1(ArgKind::BlockPos, "1.5 2 3", CONSOLE).is_err());
        assert!(parse1(ArgKind::BlockPos, "~2147483647 0 0", carol).is_err());
    }

    #[test]
    fn items_worlds_durations() {
        let mut r = FakeResolver::with_players();
        let alts = parse_arg(&spec(ArgKind::Item), "diamond 5 x", 0, &CONSOLE, &mut r).unwrap();
        assert_eq!(alts.len(), 2);
        assert_eq!(
            alts[0].0,
            ArgValue::Item(ItemStack::new("minecraft:diamond", 5))
        );
        assert_eq!(alts[0].1, " x");
        assert_eq!(
            alts[1].0,
            ArgValue::Item(ItemStack::new("minecraft:diamond", 1))
        );
        assert_eq!(alts[1].1, " 5 x");
        assert!(parse1(ArgKind::Item, "Diamond", CONSOLE).is_err());
        assert_eq!(
            parse1(ArgKind::Item, "mod:thing/x", CONSOLE).unwrap(),
            ArgValue::Item(ItemStack::new("mod:thing/x", 1))
        );
        assert_eq!(
            parse1(ArgKind::World, "the_nether", CONSOLE),
            Ok(ArgValue::World("minecraft:the_nether".into()))
        );
        assert!(
            parse1(ArgKind::World, "the_end", CONSOLE)
                .unwrap_err()
                .contains("unknown world")
        );
        for (s, t) in [
            ("10s", 200),
            ("5m", 6000),
            ("2h", 144_000),
            ("20t", 20),
            ("1d", 1_728_000),
            ("1h30m", 108_000),
            ("40", 40),
        ] {
            assert_eq!(parse_duration(s), Ok(t), "{s}");
        }
        for bad in ["", "s", "10x", "10", "99999999999999999999d"] {
            if bad == "10" {
                continue;
            }
            assert!(parse_duration(bad).is_err(), "{bad}");
        }
        assert!(parse_duration("5m3").is_err());
    }

    #[test]
    fn suggestions() {
        let mut r = FakeResolver::with_players();
        let mut s = spec(ArgKind::Player);
        s.suggestions = vec!["fixed".into()];
        let out = suggest(&s, "", &mut r);
        assert_eq!(out, ["fixed", "Alice", "Bob", "Carol"]);
        let out = suggest(&spec(ArgKind::Players), "@", &mut r);
        assert!(out.contains(&"@a".to_owned()));
        assert_eq!(
            suggest(&spec(ArgKind::Boolean), "", &mut r),
            ["true", "false"]
        );
        assert_eq!(suggest(&spec(ArgKind::World), "", &mut r).len(), 2);
        assert_eq!(suggest(&spec(ArgKind::BlockPos), "", &mut r), ["~"]);
    }
}
