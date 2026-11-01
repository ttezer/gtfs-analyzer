use std::collections::{BTreeMap, BTreeSet};

/// Call-local summary of findings skipped at their production site and derived only from
/// the DQ_016 root. Carries the audit count and rule set to K7 without allocating a full
/// `Notice`.
#[derive(Debug, Default, Clone)]
pub struct WhitespaceSuppressions {
    by_file: BTreeMap<String, BTreeMap<String, u64>>,
}

impl WhitespaceSuppressions {
    pub fn record(&mut self, file: &str, rule_id: &str) {
        self.record_n(file, rule_id, 1);
    }

    pub fn record_n(&mut self, file: &str, rule_id: &str, count: u64) {
        if count == 0 {
            return;
        }
        if let Some(rules) = self.by_file.get_mut(file) {
            if let Some(total) = rules.get_mut(rule_id) {
                *total += count;
            } else {
                rules.insert(rule_id.to_string(), count);
            }
        } else {
            let mut rules = BTreeMap::new();
            rules.insert(rule_id.to_string(), count);
            self.by_file.insert(file.to_string(), rules);
        }
    }

    pub fn merge(&mut self, other: Self) {
        for (file, rules) in other.by_file {
            for (rule_id, count) in rules {
                self.record_n(&file, &rule_id, count);
            }
        }
    }

    pub(crate) fn audit_for(&self, file: &str) -> Option<(u64, BTreeSet<String>)> {
        let rules = self.by_file.get(file)?;
        Some((
            rules.values().copied().sum(),
            rules.keys().cloned().collect(),
        ))
    }
}
