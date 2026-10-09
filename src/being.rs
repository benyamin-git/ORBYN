#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Being {
    Orb,
    Carrion,
}

impl Being {
    pub fn name(self) -> &'static str {
        match self {
            Being::Orb => "orb",
            Being::Carrion => "carrion",
        }
    }

    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Being::Orb => &["orb"],
            Being::Carrion => &["carrion"],
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Being::Orb => "wireframe globe",
            Being::Carrion => "flesh creature",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        Self::all().into_iter().find(|being| {
            being
                .aliases()
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value))
        })
    }

    pub fn all() -> [Self; 2] {
        [Being::Orb, Being::Carrion]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_table_is_complete() {
        let all = Being::all();
        assert_eq!(all, [Being::Orb, Being::Carrion]);
        for being in all {
            assert!(!being.name().is_empty());
            assert!(!being.description().is_empty());
        }
    }

    #[test]
    fn aliases_are_expected() {
        assert_eq!(Being::Orb.aliases(), ["orb"]);
        assert_eq!(Being::Carrion.aliases(), ["carrion"]);
    }

    #[test]
    fn from_name_accepts_known_names_case_insensitively() {
        assert_eq!(Being::from_name("orb"), Some(Being::Orb));
        assert_eq!(Being::from_name("ORB"), Some(Being::Orb));
        assert_eq!(Being::from_name("carrion"), Some(Being::Carrion));
    }

    #[test]
    fn from_name_rejects_unknown_and_empty() {
        assert_eq!(Being::from_name("serpent"), None);
        assert_eq!(Being::from_name("dragon"), None);
        assert_eq!(Being::from_name(""), None);
    }
}
