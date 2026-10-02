//! Правила Mojang: «эта библиотека или аргумент только для такой-то ОС или
//! функции». Лаунчер работает только под Windows x64 (раздел 2 ТЗ).

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: Action,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    // Поле `version` (регулярка версии ОС) не проверяем: оно встречается только
    // в паре с name = windows для флагов «-Dos.name=Windows 10», безвредных на 11.
}

/// Включённые функции запуска: демо-режим, своё разрешение окна, быстрый вход.
#[derive(Debug, Clone, Default)]
pub struct Features {
    pub enabled: Vec<String>,
}

impl Features {
    fn has(&self, name: &str) -> bool {
        self.enabled.iter().any(|f| f == name)
    }
}

impl OsRule {
    fn matches(&self) -> bool {
        let name_ok = self.name.as_deref().is_none_or(|n| n == "windows");
        let arch_ok = self
            .arch
            .as_deref()
            .is_none_or(|a| a == "x86_64" || a == "x64");
        name_ok && arch_ok
    }
}

impl Rule {
    fn matches(&self, features: &Features) -> bool {
        let os_ok = self.os.as_ref().is_none_or(OsRule::matches);
        let features_ok = self
            .features
            .as_ref()
            .is_none_or(|f| f.iter().all(|(name, want)| features.has(name) == *want));
        os_ok && features_ok
    }
}

/// Разрешено ли по правилам. Без правил — разрешено. С правилами — запрещено,
/// пока подходящее правило не разрешит; последнее подходящее правило главнее.
pub fn allowed(rules: &[Rule], features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut result = false;
    for rule in rules {
        if rule.matches(features) {
            result = rule.action == Action::Allow;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn no_rules_means_allowed() {
        assert!(allowed(&[], &Features::default()));
    }

    #[test]
    fn os_rules() {
        let none = Features::default();
        assert!(allowed(
            &rules(r#"[{"action":"allow","os":{"name":"windows"}}]"#),
            &none
        ));
        assert!(!allowed(
            &rules(r#"[{"action":"allow","os":{"name":"osx"}}]"#),
            &none
        ));
        assert!(allowed(
            &rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#),
            &none
        ));
        assert!(!allowed(
            &rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#),
            &none
        ));
    }

    #[test]
    fn feature_rules() {
        let demo = rules(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#);
        assert!(!allowed(&demo, &Features::default()));
        let on = Features {
            enabled: vec!["is_demo_user".into()],
        };
        assert!(allowed(&demo, &on));
    }
}
