//! "Program → profile" rules for automatic switching.
//!
//! Text form (registry and IPC): `exe|profile/exe|profile`. Windows file names cannot
//! contain `|` or `/`, so no escaping is needed.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Lowercase exe name, e.g. `valorant-win64-shipping.exe`.
    pub exe: String,
    /// 0-based onboard profile.
    pub profile: u8,
}

/// When a rule counts as active.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleMode {
    /// The program is running (checked periodically).
    #[default]
    Running,
    /// The program owns the foreground window (event driven).
    Focused,
}

impl RuleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleMode::Running => "running",
            RuleMode::Focused => "focused",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(RuleMode::Running),
            "focused" => Some(RuleMode::Focused),
            _ => None,
        }
    }
}

/// Highest profile count of any supported mouse; the real count comes from the device.
pub const MAX_PROFILES: u8 = 4;

pub fn encode(rules: &[Rule]) -> String {
    let mut out = String::new();
    for (i, r) in rules.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(&r.exe);
        out.push('|');
        out.push(char::from(b'0' + r.profile));
    }
    out
}

/// Skips invalid entries instead of failing: the text comes from outside (registry, panel).
pub fn parse(text: &str) -> Vec<Rule> {
    let mut rules: Vec<Rule> = Vec::new();
    for item in text.split('/') {
        let Some((exe, profile)) = item.rsplit_once('|') else { continue };
        let exe = exe.trim().to_lowercase();
        let Ok(profile) = profile.trim().parse::<u8>() else { continue };
        if exe.is_empty() || profile >= MAX_PROFILES || rules.iter().any(|r| r.exe == exe) {
            continue;
        }
        rules.push(Rule { exe, profile });
    }
    rules
}

/// Case-insensitive comparison of a UTF-16 exe name with a lowercase rule name, without allocating.
pub fn exe_matches(name: &[u16], lower: &str) -> bool {
    char::decode_utf16(name.iter().copied())
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .flat_map(char::to_lowercase)
        .eq(lower.chars())
}

/// Index of the first rule matching `name`.
pub fn rule_for(rules: &[Rule], name: &[u16]) -> Option<usize> {
    rules.iter().position(|r| exe_matches(name, &r.exe))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn round_trip() {
        let rules = vec![
            Rule { exe: "valorant-win64-shipping.exe".into(), profile: 1 },
            Rule { exe: "my game.exe".into(), profile: 3 },
        ];
        assert_eq!(parse(&encode(&rules)), rules);
    }

    #[test]
    fn skips_invalid_and_duplicates() {
        let rules = parse("a.exe|1/|1/b.exe|9/A.EXE|0/c.exe|x/d.exe|0");
        assert_eq!(
            rules,
            vec![Rule { exe: "a.exe".into(), profile: 1 }, Rule { exe: "d.exe".into(), profile: 0 }]
        );
    }

    #[test]
    fn first_matching_rule_is_found() {
        let rules = parse("a.exe|1/b.exe|2");
        assert_eq!(rule_for(&rules, &w("B.EXE")), Some(1));
        assert_eq!(rule_for(&rules, &w("c.exe")), None);
    }

    #[test]
    fn matches_non_ascii_case_insensitively() {
        assert!(exe_matches(&w("ÉLAN.EXE"), "élan.exe"));
        assert!(!exe_matches(&w("elan.exe"), "élan.exe"));
    }
}
