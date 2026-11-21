//! Approval policy engine. Pure functions: given the stored overrides and what
//! the server says about a tool, decide allow / ask / deny.
//!
//! Defaults: read-only tools are allowed (the demo `ping` and `fs.read`), every
//! other tool asks. An explicit override always wins.

use std::collections::BTreeMap;

use crate::registry::Decision;

pub fn key(server: &str, tool: &str) -> String {
    format!("{server}__{tool}")
}

pub fn decision(
    policies: &BTreeMap<String, Decision>,
    server: &str,
    tool: &str,
    read_only: bool,
) -> Decision {
    if let Some(stored) = policies.get(&key(server, tool)) {
        return *stored;
    }
    if read_only {
        Decision::Allow
    } else {
        Decision::Ask
    }
}

/// `tools/list` and the handshake are never gated: they have no side effects.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_defaults_to_allow() {
        let policies = BTreeMap::new();
        assert_eq!(
            decision(&policies, "demo-ping", "ping", true),
            Decision::Allow
        );
        assert_eq!(
            decision(&policies, "demo-fs", "fs.read", true),
            Decision::Allow
        );
    }

    #[test]
    fn mutating_tools_default_to_ask() {
        let policies = BTreeMap::new();
        assert_eq!(
            decision(&policies, "demo-exec", "shell.run", false),
            Decision::Ask
        );
    }

    #[test]
    fn explicit_deny_beats_a_read_only_hint() {
        let mut policies = BTreeMap::new();
        policies.insert(key("demo-fs", "fs.read"), Decision::Deny);
        assert_eq!(
            decision(&policies, "demo-fs", "fs.read", true),
            Decision::Deny
        );
    }

    #[test]
    fn explicit_allow_beats_the_ask_default() {
        let mut policies = BTreeMap::new();
        policies.insert(key("demo-exec", "shell.run"), Decision::Allow);
        assert_eq!(
            decision(&policies, "demo-exec", "shell.run", false),
            Decision::Allow
        );
    }

    #[test]
    fn overrides_are_per_tool_not_per_server() {
        let mut policies = BTreeMap::new();
        policies.insert(key("demo-ping", "echo"), Decision::Deny);
        assert_eq!(
            decision(&policies, "demo-ping", "echo", true),
            Decision::Deny
        );
        assert_eq!(
            decision(&policies, "demo-ping", "ping", true),
            Decision::Allow
        );
    }

}
