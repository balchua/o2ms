#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchStrategy {
    pub use_fork: bool,
    pub use_patch_override: bool,
}

impl Default for PatchStrategy {
    fn default() -> Self {
        Self {
            use_fork: true,
            use_patch_override: true,
        }
    }
}

impl PatchStrategy {
    #[must_use]
    pub fn summary(&self) -> &'static str {
        "Prefer a small maintained fork or patch.crates-io override for custom claims hooks."
    }
}

#[cfg(test)]
mod tests {
    use super::PatchStrategy;

    #[test]
    fn describes_patch_strategy() {
        let strategy = PatchStrategy::default();

        assert!(strategy.use_fork);
        assert!(strategy.use_patch_override);
        assert!(strategy.summary().contains("patch.crates-io"));
    }
}
