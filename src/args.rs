use crate::term::ColorMode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub fps: u32,
    pub speed: f32,
    pub size: Option<f32>,
    pub trail: f32,
    pub color: ColorMode,
    pub hud: u32,
    pub seed: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fps: 30,
            speed: 1.0,
            size: None,
            trail: 0.82,
            color: ColorMode::Auto,
            hud: 6,
            seed: None,
        }
    }
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if !(1..=240).contains(&self.fps) {
            return Err(format!("--fps must be 1-240, got {}", self.fps));
        }
        if !(0.05..=20.0).contains(&self.speed) {
            return Err(format!("--speed must be 0.05-20.0, got {}", self.speed));
        }
        if let Some(size) = self.size {
            if !(1.0..=60.0).contains(&size) {
                return Err(format!("--size must be 1-60 rows, got {size}"));
            }
        }
        if !(0.0..=0.97).contains(&self.trail) {
            return Err(format!("--trail must be 0.0-0.97, got {}", self.trail));
        }
        if self.hud > 64 {
            return Err(format!("--hud must be 0-64, got {}", self.hud));
        }
        Ok(())
    }
}

pub enum Action {
    Run(Config),
    Help,
    Version,
}

pub fn usage() -> String {
    format!(
        "\
ORBYN {VERSION}
A blue AI wireframe orb that roams your terminal to stop screen burn-in.

USAGE:
    orbyn [OPTIONS]

OPTIONS:
    -s, --speed <FLOAT>   Motion speed multiplier [default: 1.0]
        --fps <N>         Frames per second, 1-240 [default: 30]
        --size <ROWS>     Globe radius in text rows [default: auto]
        --trail <FLOAT>   Trail persistence, 0.0-0.97, higher = longer glow [default: 0.82]
        --color <MODE>    auto, truecolor, 256, 16, mono [default: auto]
        --hud <N>         Drifting telemetry fragments, 0-64 [default: 6]
        --no-hud          Disable telemetry fragments
        --seed <N>        Fixed RNG seed for reproducible motion
    -h, --help            Print this help
    -V, --version         Print version

KEYS:
    q, Ctrl-C   quit
    space       pause
    h           toggle telemetry
    +, -        speed up / slow down
"
    )
}

pub fn parse<I>(args: I) -> Result<Action, String>
where
    I: IntoIterator<Item = String>,
{
    let mut it = args.into_iter();
    let mut cfg = Config::default();

    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (arg, None),
        };

        match name.as_str() {
            "-h" | "--help" => return Ok(Action::Help),
            "-V" | "--version" => return Ok(Action::Version),
            "--no-hud" => {
                cfg.hud = 0;
                continue;
            }
            _ => {}
        }

        if !name.starts_with('-') {
            return Err(format!("unexpected argument '{name}'"));
        }

        let value = match inline {
            Some(value) => value,
            None => it
                .next()
                .ok_or_else(|| format!("missing value for {name}"))?,
        };

        match name.as_str() {
            "-s" | "--speed" => cfg.speed = parse_f32(&name, &value)?,
            "--fps" => cfg.fps = parse_u32(&name, &value)?,
            "--size" => cfg.size = Some(parse_f32(&name, &value)?),
            "--trail" => cfg.trail = parse_f32(&name, &value)?,
            "--color" => cfg.color = ColorMode::parse(&value)?,
            "--hud" => cfg.hud = parse_u32(&name, &value)?,
            "--seed" => cfg.seed = Some(parse_u64(&name, &value)?),
            _ => return Err(format!("unknown option '{name}'")),
        }
    }

    cfg.validate()?;
    Ok(Action::Run(cfg))
}

fn parse_f32(name: &str, value: &str) -> Result<f32, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value '{value}' for {name}"))
}

fn parse_u32(name: &str, value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value '{value}' for {name}"))
}

fn parse_u64(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value '{value}' for {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Action, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    fn config(args: &[&str]) -> Config {
        match run(args).expect("valid args") {
            Action::Run(cfg) => cfg,
            _ => panic!("expected run action"),
        }
    }

    #[test]
    fn defaults_are_sane() {
        let cfg = config(&[]);
        assert_eq!(cfg.fps, 30);
        assert_eq!(cfg.speed, 1.0);
        assert_eq!(cfg.hud, 6);
        assert_eq!(cfg.trail, 0.82);
        assert!(cfg.size.is_none());
        assert!(cfg.seed.is_none());
        assert_eq!(cfg.color, ColorMode::Auto);
    }

    #[test]
    fn long_and_short_options_parse() {
        let cfg = config(&[
            "-s",
            "2.5",
            "--fps",
            "60",
            "--size",
            "12",
            "--trail=0.5",
            "--color",
            "256",
            "--hud",
            "3",
            "--seed",
            "99",
        ]);
        assert_eq!(cfg.speed, 2.5);
        assert_eq!(cfg.fps, 60);
        assert_eq!(cfg.size, Some(12.0));
        assert_eq!(cfg.trail, 0.5);
        assert_eq!(cfg.color, ColorMode::Ansi256);
        assert_eq!(cfg.hud, 3);
        assert_eq!(cfg.seed, Some(99));
    }

    #[test]
    fn no_hud_sets_zero() {
        assert_eq!(config(&["--no-hud"]).hud, 0);
    }

    #[test]
    fn help_and_version_short_circuit() {
        assert!(matches!(run(&["--help"]), Ok(Action::Help)));
        assert!(matches!(run(&["-h"]), Ok(Action::Help)));
        assert!(matches!(run(&["--version"]), Ok(Action::Version)));
        assert!(matches!(run(&["-V"]), Ok(Action::Version)));
    }

    #[test]
    fn invalid_input_is_rejected() {
        assert!(run(&["--fps", "0"]).is_err());
        assert!(run(&["--fps", "241"]).is_err());
        assert!(run(&["--fps", "abc"]).is_err());
        assert!(run(&["--speed", "0"]).is_err());
        assert!(run(&["--trail", "1.5"]).is_err());
        assert!(run(&["--size", "0"]).is_err());
        assert!(run(&["--color", "rainbow"]).is_err());
        assert!(run(&["--bogus"]).is_err());
        assert!(run(&["stray"]).is_err());
        assert!(run(&["--fps"]).is_err());
    }

    #[test]
    fn usage_mentions_binary_name_and_keys() {
        let text = usage();
        assert!(text.contains("orbyn"));
        assert!(text.contains("--fps"));
        assert!(text.contains("Ctrl-C"));
    }
}
