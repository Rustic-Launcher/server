//! Usage strings: the compact way to declare a command's arguments.
//!
//! ```text
//! usage    := node (' '+ node)*
//! node     := literal | '<' arg '>' | '[' arg ']'      ([..] = optional; only trailing)
//! arg      := name [':' kind] [range] [suggest] ['*']
//! kind     := word | string | greedy | integer | number | boolean | player | players
//!           | position | block-pos | item | world | duration      (default: word)
//! range    := '(' [min] '..' [max] ')'                  (integer and number only, inclusive)
//! suggest  := '{' value ('|' value)* '}'                (fixed client-side suggestions)
//! '*'      := ask the command's completer for suggestions (dynamic)
//! ```
//!
//! Examples: `"<target:player> [amount:integer(1..64)]"`, `"set <name:word*>"`,
//! `"<mode:word{survival|creative}>"`, `"<message:greedy>"`.
//!
//! A usage with `k` optional arguments becomes `k + 1` WIT usages (WIT has no optional nodes).

use crate::bindings::rustic::api::commands::{ArgKind, ArgSpec, NodeSpec};

/// Parses one usage string into its expansions (shortest first).
pub(crate) fn parse_usage(usage: &str) -> Result<Vec<Vec<NodeSpec>>, String> {
    let mut nodes: Vec<NodeSpec> = Vec::new();
    let mut first_optional: Option<usize> = None;
    let mut names: Vec<String> = Vec::new();

    let tokens = tokenize(usage);
    let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
    for (i, tok) in tokens.iter().enumerate() {
        let (body, optional) = if let Some(b) = tok.strip_prefix('<') {
            let b = b
                .strip_suffix('>')
                .ok_or_else(|| format!("`{tok}`: missing `>`"))?;
            (Some(b), false)
        } else if let Some(b) = tok.strip_prefix('[') {
            let b = b
                .strip_suffix(']')
                .ok_or_else(|| format!("`{tok}`: missing `]`"))?;
            (Some(b), true)
        } else {
            (None, false)
        };

        if optional {
            first_optional.get_or_insert(i);
        } else if first_optional.is_some() {
            return Err(format!(
                "`{tok}`: only trailing arguments can be optional ([..] must come last)"
            ));
        }

        match body {
            None => {
                if tok.contains(['<', '>', '[', ']', '{', '}']) {
                    return Err(format!("`{tok}`: malformed literal"));
                }
                nodes.push(NodeSpec::Literal((*tok).to_owned()));
            }
            Some(body) => {
                let spec = parse_arg(body).map_err(|e| format!("`{tok}`: {e}"))?;
                if names.contains(&spec.name) {
                    return Err(format!("argument `{}` declared twice", spec.name));
                }
                names.push(spec.name.clone());
                if spec.kind == ArgKind::Greedy && i + 1 != tokens.len() {
                    return Err(format!("`{tok}`: a greedy argument must be last"));
                }
                nodes.push(NodeSpec::Argument(spec));
            }
        }
    }

    let start = first_optional.unwrap_or(nodes.len());
    Ok((start..=nodes.len()).map(|n| nodes[..n].to_vec()).collect())
}

/// Splits on whitespace outside `{...}` and `(...)`, so `{a | b}` stays one token.
fn tokenize(usage: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    for c in usage.chars() {
        match c {
            '{' | '(' => depth += 1,
            '}' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_arg(body: &str) -> Result<ArgSpec, String> {
    let mut rest = body;
    let dynamic = if let Some(r) = rest.strip_suffix('*') {
        rest = r;
        true
    } else {
        false
    };
    let mut suggestions = Vec::new();
    if let Some(open) = rest.find('{') {
        let inner = rest[open + 1..]
            .strip_suffix('}')
            .ok_or("suggestions must end with `}`")?;
        suggestions = inner
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        rest = &rest[..open];
    }
    let (mut min, mut max) = (None, None);
    if let Some(open) = rest.find('(') {
        let inner = rest[open + 1..]
            .strip_suffix(')')
            .ok_or("a range must end with `)`")?;
        let (lo, hi) = inner
            .split_once("..")
            .ok_or("a range looks like `(1..64)`, `(0..)` or `(..10)`")?;
        let hi = hi.strip_prefix('=').unwrap_or(hi);
        let num = |s: &str| -> Result<Option<f64>, String> {
            let s = s.trim();
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse::<f64>()
                    .map(Some)
                    .map_err(|_| format!("bad range bound `{s}`"))
            }
        };
        min = num(lo)?;
        max = num(hi)?;
        if let (Some(a), Some(b)) = (min, max)
            && a > b
        {
            return Err(format!("empty range {a}..{b}"));
        }
        rest = &rest[..open];
    }
    let (name, kind) = match rest.split_once(':') {
        Some((n, k)) => (n.trim(), parse_kind(k.trim())?),
        None => (rest.trim(), ArgKind::Word),
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "bad argument name `{name}` (letters, digits, `_`, `-`)"
        ));
    }
    if (min.is_some() || max.is_some()) && !matches!(kind, ArgKind::Integer | ArgKind::Number) {
        return Err("a range only applies to integer and number arguments".to_owned());
    }
    Ok(ArgSpec {
        name: name.to_owned(),
        kind,
        min,
        max,
        suggestions,
        dynamic_suggestions: dynamic,
    })
}

fn parse_kind(k: &str) -> Result<ArgKind, String> {
    Ok(match k.to_ascii_lowercase().replace('_', "-").as_str() {
        "word" => ArgKind::Word,
        "string" | "str" | "quoted" => ArgKind::String,
        "greedy" | "text" | "message" => ArgKind::Greedy,
        "integer" | "int" | "long" => ArgKind::Integer,
        "number" | "double" | "float" | "decimal" => ArgKind::Number,
        "boolean" | "bool" => ArgKind::Boolean,
        "player" => ArgKind::Player,
        "players" => ArgKind::Players,
        "position" | "pos" | "location" => ArgKind::Position,
        "block-pos" | "blockpos" => ArgKind::BlockPos,
        "item" => ArgKind::Item,
        "world" | "dimension" => ArgKind::World,
        "duration" | "time" => ArgKind::Duration,
        other => return Err(format!("unknown argument kind `{other}`")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(n: &NodeSpec) -> &ArgSpec {
        match n {
            NodeSpec::Argument(a) => a,
            NodeSpec::Literal(l) => panic!("literal {l}"),
        }
    }

    #[test]
    fn empty_usage() {
        assert_eq!(parse_usage("").unwrap(), vec![Vec::<NodeSpec>::new()]);
    }

    #[test]
    fn required_and_literals() {
        let u = parse_usage("set <name:word> <amount:integer(1..=64)>").unwrap();
        assert_eq!(u.len(), 1);
        let nodes = &u[0];
        assert_eq!(nodes[0], NodeSpec::Literal("set".into()));
        let a = arg(&nodes[2]);
        assert_eq!(a.name, "amount");
        assert_eq!(a.kind, ArgKind::Integer);
        assert_eq!((a.min, a.max), (Some(1.0), Some(64.0)));
        assert!(!a.dynamic_suggestions);
    }

    #[test]
    fn optional_expansion() {
        let u = parse_usage("<target:player> [amount:int] [reason:greedy]").unwrap();
        assert_eq!(u.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 2, 3]);
        let u = parse_usage("[target:player]").unwrap();
        assert_eq!(u.iter().map(Vec::len).collect::<Vec<_>>(), vec![0, 1]);
    }

    #[test]
    fn suggestions_dynamic_default_kind() {
        let u = parse_usage("<mode:word{survival | creative}> <home*> <n:number(0.5..)>").unwrap();
        let n = &u[0];
        assert_eq!(arg(&n[0]).suggestions, vec!["survival", "creative"]);
        assert_eq!(arg(&n[1]).kind, ArgKind::Word);
        assert!(arg(&n[1]).dynamic_suggestions);
        assert_eq!((arg(&n[2]).min, arg(&n[2]).max), (Some(0.5), None));
    }

    #[test]
    fn kinds() {
        for (k, want) in [
            ("string", ArgKind::String),
            ("text", ArgKind::Greedy),
            ("bool", ArgKind::Boolean),
            ("players", ArgKind::Players),
            ("pos", ArgKind::Position),
            ("block_pos", ArgKind::BlockPos),
            ("item", ArgKind::Item),
            ("world", ArgKind::World),
            ("duration", ArgKind::Duration),
            ("double", ArgKind::Number),
        ] {
            let u = parse_usage(&format!("<x:{k}>")).unwrap();
            assert_eq!(arg(&u[0][0]).kind, want, "{k}");
        }
    }

    #[test]
    fn errors() {
        for bad in [
            "[a] <b>",
            "[a] lit",
            "<a:greedy> <b>",
            "<a:nope>",
            "<a> <a>",
            "<a:word(1..2)>",
            "<a:int(5..1)>",
            "<a:int(x..1)>",
            "<a",
            "[a",
            "<:word>",
            "<a b>",
            "li<t",
            "<a:word{x|y>",
        ] {
            assert!(parse_usage(bad).is_err(), "{bad} should fail");
        }
    }
}
