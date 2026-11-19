use crate::ast::{Dnf, Rule, Severity};

#[derive(Clone, Debug)]
pub struct NormalizedRule {
    pub severity: Severity,
    pub message: String,
    pub when: Dnf,
}

impl From<&Rule> for NormalizedRule {
    fn from(rule: &Rule) -> Self {
        Self {
            severity: rule.severity,
            message: rule.message.clone(),
            when: Dnf::from(rule.when.clone()),
        }
    }
}

#[cfg(test)]
impl NormalizedRule {
    pub fn new(severity: Severity, message: &str, when: Dnf) -> Self {
        Self {
            severity,
            message: message.to_owned(),
            when,
        }
    }
}
