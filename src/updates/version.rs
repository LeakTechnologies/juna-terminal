//! Strict versions emitted by Juna's release cadence, ordered independently of API list order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    core: [u64; 3],
    dev: Option<u64>,
}
impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let (core, dev) = match text.split_once("-dev") {
            Some((core, dev)) => (core, Some(number(dev)?)),
            None => (text, None),
        };
        let parts = core.split('.').map(number).collect::<Option<Vec<_>>>()?;
        if parts.len() != 3 || dev == Some(0) {
            return None;
        }
        Some(Self {
            core: parts.try_into().ok()?,
            dev,
        })
    }
    pub fn preview(&self) -> bool {
        self.dev.is_some()
    }
}
fn number(text: &str) -> Option<u64> {
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|c| c.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return None;
    }
    text.parse().ok()
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.dev, other.dev) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn stable_and_development_order_are_numeric() {
        use super::Version as V;
        assert!(V::parse("0.1.1-dev10") > V::parse("0.1.1-dev9"));
        assert!(V::parse("0.1.1") > V::parse("0.1.1-dev999"));
        assert!(V::parse("0.2.0-dev1") > V::parse("0.1.1"));
        for invalid in [
            "v0.1.1",
            "0.1",
            "0.01.1",
            "0.1.1-dev0",
            "0.1.1-beta1",
            "0.1.1-dev1+stuff",
            "0.1.1\n",
        ] {
            assert!(V::parse(invalid).is_none());
        }
    }
}
