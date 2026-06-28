//! Faithful port of `server/src/services/artistParser.ts`.
//!
//! 1. Temporarily replace known exception names with placeholders.
//! 2. Split on the combined delimiter regex.
//! 3. Restore placeholders.
//! 4. Trim, de-duplicate (case-insensitive), assign role + position.

use regex::Regex;

use once_cell::sync::Lazy;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedArtist {
    pub name: String,
    pub normalized_name: String,
    pub role: String, // "primary" | "featured"
    pub position: usize,
}

/// Combined delimiter regex matching the TS `DELIMITER_RE`.
///
/// In TS this is a global regex used with `.split()`. Here we build a single
/// regex and use `.split()` on it directly (regex crate supports `split`).
static DELIMITER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\s*;\s*|\s*/\s*|\s*,\s*|\s+[Ff][Ee][Aa][Tt]\.?\s+|\s+[Ff][Tt]\.?\s+|\s+[Ff][Ee][Aa][Tt][Uu][Rr][Ii][Nn][Gg]\s+|\s+&\s*|\s+[Aa][Nn][Dd]\s+|\s+x\s+"
    )
    .expect("invalid delimiter regex")
});

/// Exception names that must NOT be split. Embedded as a default list so the
/// binary is self-contained (no external JSON needed at runtime).
const DEFAULT_EXCEPTIONS: &[&str] = &[
    "Tyler, The Creator",
    "Earth, Wind & Fire",
    "Crosby, Stills & Nash",
    "Crosby, Stills, Nash & Young",
    "Simon & Garfunkel",
    "Yung Lean & Sad Boys",
    "Sophie Ellis-Bextor",
    "Florence + The Machine",
    "DC The Don",
    "PJ The Human",
    "Anderson .Paak",
    "A$AP Rocky",
    "A$AP Mob",
    "Migos",
    "City Morgue",
    "Paris, Texas",
    "Kids See Ghosts",
    "Childish Gambino",
    "Lil Uzi Vert",
    "Lil Nas X",
    "Lil Baby",
    "Lil Durk",
    "Lil Yachty",
    "Lil Pump",
    "Lil Wayne",
    "Lil Tjay",
    "Playboi Carti",
    "Chief Keef",
    "Young Thug",
    "YoungBoy Never Broke Again",
    "Young Nudy",
    "21 Savage",
    "Summer Walker",
    "Summer Salt",
    "Marwan Pablo",
    "Marwan Moussa",
    "Abyusif",
    "Afroto",
    "Wegz",
    "Ahmed Santa",
    "Ray-Ban",
    "Malcolm X",
];

pub fn load_exceptions() -> Vec<String> {
    DEFAULT_EXCEPTIONS.iter().map(|s| s.trim().to_string()).collect()
}

pub fn normalize_artist_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let collapsed: String = {
        let mut out = String::with_capacity(lower.len());
        let mut prev_space = false;
        for ch in lower.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    out.push(' ');
                }
                prev_space = true;
            } else {
                out.push(ch);
                prev_space = false;
            }
        }
        out
    };
    collapsed.trim().to_string()
}

fn escape_regex(s: &str) -> String {
    regex::escape(s)
}

pub fn split_artists(raw: Option<&str>) -> Vec<ParsedArtist> {
    split_artists_with(raw, None)
}

pub fn split_artists_with(raw: Option<&str>, custom_exceptions: Option<&[String]>) -> Vec<ParsedArtist> {
    let raw = match raw {
        Some(r) => r.trim(),
        None => return Vec::new(),
    };
    if raw.is_empty() {
        return Vec::new();
    }

    let mut exceptions: Vec<String> = match custom_exceptions {
        Some(c) => c.iter().map(|s| s.trim().to_string()).collect(),
        None => load_exceptions(),
    };
    exceptions.retain(|s| !s.is_empty());
    // longest first for overlap safety
    exceptions.sort_by_key(|b| std::cmp::Reverse(b.len()));

    let mut working = raw.to_string();

    // protect exceptions with placeholders
    let mut placeholders: Vec<(String, String)> = Vec::new();
    for (i, exc) in exceptions.iter().enumerate() {
        let placeholder = format!("\u{0}EXC{}\u{0}", i);
        let re = Regex::new(&format!("(?i){}", escape_regex(exc))).unwrap();
        if re.is_match(&working) {
            working = re.replace_all(&working, placeholder.as_str()).to_string();
            placeholders.push((placeholder, exc.clone()));
        }
    }

    // split on delimiters
    let raw_parts: Vec<&str> = DELIMITER_RE.split(&working).collect();

    // restore placeholders + trim
    let mut parts: Vec<String> = Vec::new();
    for p in raw_parts {
        let mut restored = p.trim().to_string();
        for (token, original) in &placeholders {
            if restored.contains(token.as_str()) {
                restored = restored.replace(token.as_str(), original.as_str());
            }
        }
        let trimmed = restored.trim().to_string();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }

    if parts.is_empty() {
        return Vec::new();
    }

    // de-duplicate (case-insensitive), preserving first occurrence's casing
    let mut seen: Vec<(String, String)> = Vec::new(); // (lowercase key, original)
    for p in parts {
        let key = p.to_lowercase();
        if !seen.iter().any(|(k, _)| k == &key) {
            seen.push((key, p));
        }
    }

    seen.into_iter()
        .enumerate()
        .map(|(idx, (_, name))| ParsedArtist {
            normalized_name: normalize_artist_name(&name),
            role: if idx == 0 { "primary" } else { "featured" }.to_string(),
            position: idx,
            name,
        })
        .collect()
}

pub fn format_artist_display(artists: &[ParsedArtist]) -> String {
    if artists.is_empty() {
        return "Unknown Artist".to_string();
    }
    let primary: Vec<&str> = artists.iter().filter(|a| a.role == "primary").map(|a| a.name.as_str()).collect();
    let featured: Vec<&str> = artists.iter().filter(|a| a.role == "featured").map(|a| a.name.as_str()).collect();
    let mut result = primary.join(", ");
    if !featured.is_empty() {
        result.push_str(" feat. ");
        result.push_str(&featured.join(", "));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_basic_delimiters() {
        let r = split_artists(Some("Drake feat. Future"));
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "Drake");
        assert_eq!(r[0].role, "primary");
        assert_eq!(r[1].name, "Future");
        assert_eq!(r[1].role, "featured");
    }

    #[test]
    fn splits_amp_and_comma() {
        let r = split_artists(Some("Drake, Future & Lil Wayne"));
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn preserves_exceptions() {
        let r = split_artists(Some("Tyler, The Creator, Drake"));
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "Tyler, The Creator");
    }

    #[test]
    fn dedup_case_insensitive() {
        let r = split_artists(Some("DRAKE, drake"));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "DRAKE");
    }

    #[test]
    fn empty_input() {
        assert!(split_artists(None).is_empty());
        assert!(split_artists(Some("")).is_empty());
        assert!(split_artists(Some("   ")).is_empty());
    }
}
