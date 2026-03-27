// ─── SignalBus — Per-request signal collector ────────────────────────────────
//
// Collects signals from all engines for a single request. The injection budget
// in userprompt_context selects top-N by utility per category.
// ──────────────────────────────────────────────────────────────────────────────

use super::signal::{Signal, SignalCategory};
use std::collections::HashMap;

/// Collects signals from all engines for a single request.
pub struct SignalBus {
    signals: Vec<Signal>,
}

impl SignalBus {
    pub fn new() -> Self {
        Self {
            signals: Vec::new(),
        }
    }

    pub fn push(&mut self, signal: Signal) {
        self.signals.push(signal);
    }

    pub fn signals(&self) -> &[Signal] {
        &self.signals
    }

    pub fn drain(self) -> Vec<Signal> {
        self.signals
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Select top signal per category by utility score.
    pub fn top_per_category(&self) -> Vec<&Signal> {
        let mut best: HashMap<SignalCategory, &Signal> = HashMap::new();
        for signal in &self.signals {
            let entry = best.entry(signal.category).or_insert(signal);
            if signal.utility > entry.utility {
                *entry = signal;
            }
        }
        best.into_values().collect()
    }

    /// Select top N signals overall by utility, regardless of category.
    pub fn top_n(&self, n: usize) -> Vec<&Signal> {
        let mut sorted: Vec<&Signal> = self.signals.iter().collect();
        sorted.sort_by(|a, b| {
            b.utility
                .partial_cmp(&a.utility)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(n);
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bus() {
        let bus = SignalBus::new();
        assert!(bus.is_empty());
        assert!(bus.top_per_category().is_empty());
    }

    #[test]
    fn top_per_category_picks_highest() {
        let mut bus = SignalBus::new();
        bus.push(Signal::advisory(
            SignalCategory::Safety,
            0.3,
            "low".into(),
            "a",
        ));
        bus.push(Signal::advisory(
            SignalCategory::Safety,
            0.9,
            "high".into(),
            "b",
        ));
        bus.push(Signal::advisory(
            SignalCategory::Loop,
            0.5,
            "loop".into(),
            "c",
        ));
        let top = bus.top_per_category();
        assert_eq!(top.len(), 2);
        let safety = top
            .iter()
            .find(|s| s.category == SignalCategory::Safety)
            .unwrap();
        assert_eq!(safety.utility, 0.9);
    }

    #[test]
    fn top_n_returns_highest() {
        let mut bus = SignalBus::new();
        bus.push(Signal::advisory(
            SignalCategory::Safety,
            0.3,
            "a".into(),
            "x",
        ));
        bus.push(Signal::advisory(SignalCategory::Loop, 0.9, "b".into(), "x"));
        bus.push(Signal::advisory(
            SignalCategory::Focus,
            0.6,
            "c".into(),
            "x",
        ));
        let top = bus.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].utility, 0.9);
        assert_eq!(top[1].utility, 0.6);
    }
}
