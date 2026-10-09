use crate::args::Config;
use crate::being::Being;

pub const AUTO_INTERVAL: f32 = 300.0;

pub struct Rotation {
    pub list: Vec<Being>,
    pub interval: Option<f32>,
}

pub fn rotation_for(cfg: &Config) -> Option<Rotation> {
    cfg.playlist.as_ref().map(|pl| Rotation {
        list: pl.clone(),
        interval: cfg.rotate.or(Some(AUTO_INTERVAL)),
    })
}

pub fn resolve_start_kind(being: Option<Being>, rotation: Option<&Rotation>) -> Being {
    being
        .or_else(|| rotation.and_then(|r| r.list.first().copied()))
        .unwrap_or(Being::Orb)
}

pub fn next_after(list: &[Being], current: Being) -> Option<Being> {
    if list.is_empty() {
        return None;
    }
    let idx = list.iter().position(|&b| b == current).unwrap_or(0);
    Some(list[(idx + 1) % list.len()])
}

pub fn swap_due(elapsed: f32, interval: Option<f32>) -> bool {
    match interval {
        Some(interval) => elapsed >= interval,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Config;
    use crate::being::Being;

    fn cfg_playlist_only() -> Config {
        Config {
            playlist: Some(vec![Being::Orb, Being::Carrion]),
            ..Config::default()
        }
    }

    fn cfg_rotate_only() -> Config {
        Config {
            playlist: Some(Being::all().to_vec()),
            rotate: Some(30.0),
            ..Config::default()
        }
    }

    fn cfg_none() -> Config {
        Config::default()
    }

    #[test]
    fn rotation_for_returns_playlist_config() {
        let cfg = Config {
            playlist: Some(vec![Being::Carrion, Being::Orb]),
            rotate: Some(15.0),
            ..Config::default()
        };
        let rotation = rotation_for(&cfg).expect("rotation");
        assert_eq!(rotation.list, vec![Being::Carrion, Being::Orb]);
        assert_eq!(rotation.interval, Some(15.0));
    }

    #[test]
    fn rotation_for_playlist_alone_uses_auto_interval() {
        let rotation = rotation_for(&cfg_playlist_only()).expect("rotation");
        assert_eq!(rotation.interval, Some(AUTO_INTERVAL));
    }

    #[test]
    fn rotation_for_rotate_alone_uses_all_and_value() {
        let rotation = rotation_for(&cfg_rotate_only()).expect("rotation");
        assert_eq!(rotation.list, Being::all().to_vec());
        assert_eq!(rotation.interval, Some(30.0));
    }

    #[test]
    fn rotation_for_without_playlist_is_none() {
        assert!(rotation_for(&cfg_none()).is_none());
    }

    #[test]
    fn resolve_start_kind_explicit_wins() {
        let rotation = rotation_for(&cfg_playlist_only()).expect("rotation");
        assert_eq!(
            resolve_start_kind(Some(Being::Carrion), Some(&rotation)),
            Being::Carrion
        );
    }

    #[test]
    fn resolve_start_kind_playlist_first() {
        let rotation = rotation_for(&cfg_playlist_only()).expect("rotation");
        assert_eq!(resolve_start_kind(None, Some(&rotation)), Being::Orb);
    }

    #[test]
    fn resolve_start_kind_defaults_to_orb() {
        assert_eq!(resolve_start_kind(None, None), Being::Orb);
    }

    #[test]
    fn resolve_start_kind_empty_playlist_is_orb() {
        let rotation = Rotation {
            list: Vec::new(),
            interval: Some(AUTO_INTERVAL),
        };
        assert_eq!(resolve_start_kind(None, Some(&rotation)), Being::Orb);
    }

    #[test]
    fn next_after_advances() {
        assert_eq!(
            next_after(&[Being::Orb, Being::Carrion], Being::Orb),
            Some(Being::Carrion)
        );
    }

    #[test]
    fn next_after_wraps() {
        assert_eq!(
            next_after(&[Being::Orb, Being::Carrion], Being::Carrion),
            Some(Being::Orb)
        );
    }

    #[test]
    fn next_after_single_entry_is_itself() {
        assert_eq!(next_after(&[Being::Orb], Being::Orb), Some(Being::Orb));
    }

    #[test]
    fn next_after_empty_is_none() {
        assert_eq!(next_after(&[], Being::Orb), None);
    }

    #[test]
    fn swap_due_true_when_elapsed_past_interval() {
        assert!(swap_due(2.0, Some(1.0)));
    }

    #[test]
    fn swap_due_false_before_interval() {
        assert!(!swap_due(0.5, Some(1.0)));
    }

    #[test]
    fn swap_due_false_without_interval() {
        assert!(!swap_due(9.0, None));
    }

    #[test]
    fn swap_due_true_at_exact_interval() {
        assert!(swap_due(1.0, Some(1.0)));
    }
}
