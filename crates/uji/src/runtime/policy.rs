use std::collections::{BTreeMap, BTreeSet};

use globset::Glob;
use mlua::Value as LuaValue;
use regex::Regex;

use strum::VariantArray;
use uji_core::tools::policy::{Action, Matcher, Rule, ToolPolicy, ToolRules};

const PRECEDENCE: [Action; Action::VARIANTS.len()] = [Action::Deny, Action::Allow, Action::Ask];

pub(super) fn compile(
    rules: &BTreeMap<String, LuaValue>,
    known: &BTreeSet<String>,
) -> (ToolPolicy, Vec<String>) {
    let mut policy = ToolPolicy::default();
    let mut notices = Vec::new();
    for (name, value) in rules {
        if name == "default" {
            match value.as_string().map(mlua::LuaString::to_string_lossy) {
                Some(default) => match Action::parse(&default) {
                    Some(action) => policy.default = action,
                    None => notices.push(format!(
                        "tool policy: default `{default}` is not allow, ask or deny; asking instead"
                    )),
                },
                None => notices.push(String::from(
                    "tool policy: default must be allow, ask or deny; asking instead",
                )),
            }
            continue;
        }
        if !known.contains(name) {
            notices.push(format!(
                "tool policy: `{name}` is not a tool, so its rules do nothing"
            ));
            continue;
        }
        policy
            .tools
            .insert(name.clone(), tool_rules(name, value.clone(), &mut notices));
    }
    (policy, notices)
}

fn tool_rules(name: &str, value: LuaValue, notices: &mut Vec<String>) -> ToolRules {
    let LuaValue::Table(table) = value else {
        notices.push(format!(
            "tool policy: `{name}` is not a table of rules; asking before every {name}"
        ));
        return ToolRules {
            rules: Vec::new(),
            default: Some(Action::Ask),
        };
    };
    let mut rules = Vec::new();
    let mut default = None;
    if let Ok(Some(value)) = table.get::<Option<String>>("default") {
        default = Some(Action::parse(&value).unwrap_or_else(|| {
            notices.push(format!(
                "tool policy: `{name}` default `{value}` is not allow, ask or deny; asking instead"
            ));
            Action::Ask
        }));
    }
    let mut unreadable = false;
    for action in PRECEDENCE {
        let key: &'static str = action.into();
        if let Ok(Some(entries)) = table.get::<Option<Vec<String>>>(key) {
            for entry in entries {
                if let Some(matcher) = matcher(&entry) {
                    rules.push(Rule { matcher, action });
                } else {
                    unreadable = true;
                    notices.push(format!(
                        "tool policy: `{name}` {key} rule `{entry}` is not a valid pattern"
                    ));
                }
            }
        }
    }
    if unreadable {
        let raised = default.map_or(Action::Ask, |action| action.strictest(Action::Ask));
        if default != Some(raised) {
            notices.push(format!(
                "tool policy: asking before every {name}, because part of its policy could not be read"
            ));
        }
        default = Some(raised);
    }
    ToolRules { rules, default }
}

fn matcher(value: &str) -> Option<Matcher> {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'/' && bytes[bytes.len() - 1] == b'/' {
        let pattern = &value[1..value.len() - 1];
        return Regex::new(pattern).ok().map(Matcher::Regex);
    }
    if value.contains(['*', '?', '[']) {
        return Glob::new(value)
            .ok()
            .map(|glob| Matcher::Glob(glob.compile_matcher()));
    }
    Some(Matcher::Exact(value.to_string()))
}
