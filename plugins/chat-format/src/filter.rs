//! Anti-spam and content filters (pure, clock passed in as milliseconds).

use std::collections::{HashMap, VecDeque};

use rustic::Uuid;

/// What a filter does with a message that trips it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Cancel the message and tell the sender.
    Block,
    /// Mask the offending part (words, links) and let it through.
    #[default]
    Mask,
    /// Lower-case the message (caps filter).
    Lowercase,
}

/// Share of upper-case letters among letters, when the message has at least `min_letters`.
pub fn caps_ratio(message: &str, min_letters: usize) -> Option<f32> {
    let letters = message.chars().filter(|c| c.is_alphabetic()).count();
    if letters < min_letters.max(1) {
        return None;
    }
    let upper = message.chars().filter(|c| c.is_uppercase()).count();
    Some(upper as f32 / letters as f32)
}

/// Whether the message shouts: more than `max_ratio` capitals over at least `min_letters` letters.
pub fn is_shouting(message: &str, min_letters: usize, max_ratio: f32) -> bool {
    caps_ratio(message, min_letters).is_some_and(|r| r > max_ratio)
}

/// Top-level domains recognised in bare host names (`example.com`); URLs with a scheme or
/// `www.` and IPv4 addresses are always recognised.
const TLDS: &[&str] = &[
    "com", "net", "org", "io", "gg", "fr", "de", "uk", "co", "me", "info", "xyz", "eu", "ru", "tv",
    "be", "us", "ca", "es", "it", "nl", "ch", "dev", "app", "club", "online", "site", "fun", "pro",
    "biz", "cc", "ly", "to", "link", "shop", "store", "live",
];

/// The host of a token that looks like a link, lower-cased (`https://Foo.com/x` -> `foo.com`).
pub fn link_host(token: &str) -> Option<String> {
    let t = token.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | ',' | ';' | '!' | '?'
        )
    });
    let lower = t.to_ascii_lowercase();
    let (had_scheme, rest) = match lower.find("://") {
        Some(i) => (true, &lower[i + 3..]),
        None => (false, lower.as_str()),
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("").trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let valid_labels = labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    if !valid_labels {
        return None;
    }
    let is_ipv4 = labels.len() == 4 && labels.iter().all(|l| l.parse::<u8>().is_ok());
    let tld = labels[labels.len() - 1];
    let known_tld = TLDS.contains(&tld);
    if had_scheme || host.starts_with("www.") || is_ipv4 || known_tld {
        Some(host.to_owned())
    } else {
        None
    }
}

/// Whether `host` is `allowed` or one of its subdomains.
pub fn host_allowed(host: &str, whitelist: &[String]) -> bool {
    whitelist.iter().any(|w| {
        let w = w.trim().trim_start_matches("*.").to_ascii_lowercase();
        !w.is_empty() && (host == w || host.ends_with(&format!(".{w}")))
    })
}

/// The links of a message that are not whitelisted, as whitespace-separated tokens.
pub fn forbidden_links(message: &str, whitelist: &[String]) -> Vec<String> {
    message
        .split_whitespace()
        .filter(|tok| link_host(tok).is_some_and(|h| !host_allowed(&h, whitelist)))
        .map(str::to_owned)
        .collect()
}

/// Replaces forbidden link tokens with `replacement`.
pub fn mask_links(message: &str, whitelist: &[String], replacement: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut last = 0;
    for (start, tok) in token_spans(message) {
        if link_host(tok).is_some_and(|h| !host_allowed(&h, whitelist)) {
            out.push_str(&message[last..start]);
            out.push_str(replacement);
            last = start + tok.len();
        }
    }
    out.push_str(&message[last..]);
    out
}

fn token_spans(s: &str) -> Vec<(usize, &str)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if let Some(st) = start.take() {
                spans.push((st, &s[st..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        spans.push((st, &s[st..]));
    }
    spans
}

/// Byte ranges of the banned words found in `message` (case-insensitive substring match).
fn word_ranges(message: &str, words: &[String]) -> Vec<(usize, usize)> {
    let lower = message.to_lowercase();
    // Lower-casing may change byte lengths for some scripts; only match when it does not.
    if lower.len() != message.len() {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    for w in words {
        let w = w.trim().to_lowercase();
        if w.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(i) = lower[from..].find(&w) {
            let start = from + i;
            ranges.push((start, start + w.len()));
            from = start + w.len();
        }
    }
    ranges.sort_unstable();
    ranges
}

/// Whether the message contains a banned word.
pub fn has_banned_word(message: &str, words: &[String]) -> bool {
    !word_ranges(message, words).is_empty()
}

/// Masks banned words with `mask` (one per character).
pub fn mask_words(message: &str, words: &[String], mask: char) -> String {
    let ranges = word_ranges(message, words);
    if ranges.is_empty() {
        return message.to_owned();
    }
    let mut out = String::with_capacity(message.len());
    for (i, c) in message.char_indices() {
        if ranges.iter().any(|(s, e)| i >= *s && i < *e) {
            out.push(mask);
        } else {
            out.push(c);
        }
    }
    out
}

/// Normalised form used for duplicate detection: lower case, letters and digits only.
pub fn normalize(message: &str) -> String {
    message
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Why the spam guard refused a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpamVerdict {
    /// Fine.
    Ok,
    /// Too many messages: try again in this many seconds.
    TooFast(u64),
    /// The same message again within the window.
    Duplicate,
}

/// Rate limit (N messages per window) and duplicate detection, per player.
#[derive(Debug, Default)]
pub struct SpamGuard {
    players: HashMap<Uuid, History>,
}

#[derive(Debug, Default)]
struct History {
    times: VecDeque<u64>,
    last: Option<(String, u64)>,
}

/// Spam limits.
#[derive(Debug, Clone, Copy)]
pub struct SpamLimits {
    /// Messages allowed per window (0 = no rate limit).
    pub max_messages: usize,
    /// The window, in milliseconds.
    pub window_ms: u64,
    /// Repeating the same message within this many milliseconds is refused (0 = off).
    pub duplicate_ms: u64,
}

impl SpamGuard {
    /// Checks a message without recording it.
    pub fn check(&self, player: Uuid, message: &str, now: u64, limits: SpamLimits) -> SpamVerdict {
        let Some(h) = self.players.get(&player) else {
            return SpamVerdict::Ok;
        };
        if limits.max_messages > 0 {
            let recent: Vec<u64> = h
                .times
                .iter()
                .copied()
                .filter(|t| now.saturating_sub(*t) < limits.window_ms)
                .collect();
            if recent.len() >= limits.max_messages {
                let oldest = recent[recent.len() - limits.max_messages];
                let wait_ms = limits.window_ms.saturating_sub(now.saturating_sub(oldest));
                return SpamVerdict::TooFast(wait_ms.div_ceil(1000).max(1));
            }
        }
        if limits.duplicate_ms > 0
            && let Some((last, at)) = &h.last
        {
            let norm = normalize(message);
            if !norm.is_empty() && *last == norm && now.saturating_sub(*at) < limits.duplicate_ms {
                return SpamVerdict::Duplicate;
            }
        }
        SpamVerdict::Ok
    }

    /// Records an accepted message.
    pub fn record(&mut self, player: Uuid, message: &str, now: u64, limits: SpamLimits) {
        let h = self.players.entry(player).or_default();
        h.times.push_back(now);
        while h
            .times
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= limits.window_ms)
        {
            h.times.pop_front();
        }
        while h.times.len() > limits.max_messages.max(1) * 2 {
            h.times.pop_front();
        }
        h.last = Some((normalize(message), now));
    }

    /// Forgets a player (on quit).
    pub fn forget(&mut self, player: Uuid) {
        self.players.remove(&player);
    }

    /// Players tracked.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.players.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn caps() {
        assert!(is_shouting("HELLO EVERYONE", 6, 0.7));
        assert!(!is_shouting("Hello everyone", 6, 0.7));
        assert!(!is_shouting("OK", 6, 0.7)); // too short to judge
        assert_eq!(caps_ratio("AAbb", 1), Some(0.5));
    }

    #[test]
    fn links() {
        assert_eq!(
            link_host("https://Evil.com/x?y").as_deref(),
            Some("evil.com")
        );
        assert_eq!(
            link_host("www.thing.example").as_deref(),
            Some("www.thing.example")
        );
        assert_eq!(
            link_host("play.server.net:25565").as_deref(),
            Some("play.server.net")
        );
        assert_eq!(link_host("10.0.0.1").as_deref(), Some("10.0.0.1"));
        assert_eq!(link_host("(join.gg)").as_deref(), Some("join.gg"));
        assert_eq!(link_host("e.g."), None);
        assert_eq!(link_host("3.14"), None);
        assert_eq!(link_host("hello"), None);
        assert_eq!(link_host("file.txt"), None);
        let wl = s(&["rustic.example.org", "*.youtube.com"]);
        assert!(host_allowed("rustic.example.org", &wl));
        assert!(host_allowed("wiki.rustic.example.org", &wl));
        assert!(host_allowed("m.youtube.com", &wl));
        assert!(!host_allowed("notrustic.example.org.evil.com", &wl));
        assert!(!host_allowed("evilrustic.example.org", &wl));
        assert_eq!(
            forbidden_links("see https://rustic.example.org and evil.com", &wl),
            s(&["evil.com"])
        );
        assert_eq!(
            mask_links("go to evil.com now", &wl, "<link>"),
            "go to <link> now"
        );
    }

    #[test]
    fn words() {
        let w = s(&["heck", "darn"]);
        assert!(has_banned_word("What the HECK", &w));
        assert!(!has_banned_word("hello", &w));
        assert_eq!(
            mask_words("what the Heck, darn!", &w, '*'),
            "what the ****, ****!"
        );
        assert_eq!(mask_words("clean", &w, '*'), "clean");
        assert_eq!(mask_words("éheck", &w, '#'), "é####");
    }

    #[test]
    fn rate_limit_and_duplicates() {
        let p = Uuid::from_u128(1);
        let limits = SpamLimits {
            max_messages: 3,
            window_ms: 5_000,
            duplicate_ms: 10_000,
        };
        let mut g = SpamGuard::default();
        for (i, msg) in ["a1", "b2", "c3"].iter().enumerate() {
            assert_eq!(g.check(p, msg, i as u64 * 100, limits), SpamVerdict::Ok);
            g.record(p, msg, i as u64 * 100, limits);
        }
        assert_eq!(g.check(p, "d4", 300, limits), SpamVerdict::TooFast(5));
        assert_eq!(g.check(p, "d4", 4_500, limits), SpamVerdict::TooFast(1));
        assert_eq!(g.check(p, "d4", 5_001, limits), SpamVerdict::Ok);
        // duplicate of the last one, normalised
        assert_eq!(g.check(p, "C 3!", 6_000, limits), SpamVerdict::Duplicate);
        assert_eq!(g.check(p, "c3", 20_000, limits), SpamVerdict::Ok);
        // another player is independent
        assert_eq!(
            g.check(Uuid::from_u128(2), "a1", 300, limits),
            SpamVerdict::Ok
        );
        g.forget(p);
        assert_eq!(g.len(), 0);
    }
}
