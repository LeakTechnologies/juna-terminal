//! Native window identity shared with update preflight; never supplied by child output.
pub fn title() -> String {
    format!("Juna - v{}", env!("CARGO_PKG_VERSION"))
}
pub fn recognized(title: &str) -> bool {
    title == "Juna Terminal"
        || title
            .strip_prefix("Juna - v")
            .is_some_and(|version| crate::updates::version::Version::parse(version).is_some())
}

#[cfg(test)]
mod tests {
    #[test]
    fn versioned_and_legacy_titles_preserve_update_preflight() {
        assert_eq!(
            super::title(),
            format!("Juna - v{}", env!("CARGO_PKG_VERSION"))
        );
        assert!(super::recognized(&super::title()));
        assert!(super::recognized("Juna Terminal"));
        for bad in [
            "Juna - v",
            "Juna - v0.1.1-dev0",
            "Juna - v0.1.1-dev16 suffix",
            "Other Juna Terminal",
            "Juna - v01.1.1",
        ] {
            assert!(!super::recognized(bad));
        }
    }
}
