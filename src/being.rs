use crate::args::Config;
use crate::carrion::Carrion;
use crate::scene::Grid;
use crate::App;

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
            Being::Orb => "Blue wireframe orb",
            Being::Carrion => "Red flesh creature with dark brain folds",
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

pub enum Sim {
    Orb(Box<App>),
    Carrion(Box<Carrion>),
}

impl Sim {
    pub fn new(being: Being, cfg: &Config, w: usize, h: usize) -> Self {
        match being {
            Being::Orb => Sim::Orb(Box::new(App::new(cfg, w, h))),
            Being::Carrion => Sim::Carrion(Box::new(Carrion::new(cfg, w, h))),
        }
    }

    pub fn update(&mut self, dt: f32, w: usize, h: usize) {
        match self {
            Sim::Orb(app) => app.update(dt, w, h),
            Sim::Carrion(carrion) => carrion.update(dt, w, h),
        }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        match self {
            Sim::Orb(app) => app.resize(w, h),
            Sim::Carrion(carrion) => carrion.resize(w, h),
        }
    }

    pub fn show_too_small(&mut self, w: usize, h: usize) {
        match self {
            Sim::Orb(app) => app.show_too_small(w, h),
            Sim::Carrion(carrion) => carrion.show_too_small(w, h),
        }
    }

    pub fn toggle_overlay(&mut self) {
        match self {
            Sim::Orb(app) => app.toggle_hud(),
            Sim::Carrion(carrion) => carrion.toggle_detail(),
        }
    }

    pub fn toggle_pause(&mut self) {
        match self {
            Sim::Orb(app) => app.paused = !app.paused,
            Sim::Carrion(carrion) => carrion.toggle_pause(),
        }
    }

    pub fn paused(&self) -> bool {
        match self {
            Sim::Orb(app) => app.paused,
            Sim::Carrion(carrion) => carrion.paused(),
        }
    }

    pub fn adjust_speed(&mut self, ratio: f32) {
        match self {
            Sim::Orb(app) => app.adjust_speed(ratio),
            Sim::Carrion(carrion) => carrion.adjust_speed(ratio),
        }
    }

    pub fn grid(&self) -> &Grid {
        match self {
            Sim::Orb(app) => &app.grid,
            Sim::Carrion(carrion) => carrion.grid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim_cfg() -> Config {
        Config {
            seed: Some(7),
            ..Config::default()
        }
    }

    #[test]
    fn sim_new_orb_builds_grid() {
        let cfg = sim_cfg();
        assert_eq!(Sim::new(Being::Orb, &cfg, 30, 10).grid().w, 30);
    }

    #[test]
    fn sim_new_carrion_builds_grid() {
        let cfg = sim_cfg();
        let sim = Sim::new(Being::Carrion, &cfg, 30, 10);
        assert_eq!(sim.grid().w, 30);
        assert_eq!(sim.grid().h, 10);
    }

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
