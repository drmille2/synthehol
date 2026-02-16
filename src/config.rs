use serde::Deserialize;
use std::path::Path;
use std::{fmt, fs, io};

use crate::monitor::MonitorArgs;
use crate::reporters::pagerduty::PagerdutyReporterArgs;
use crate::reporters::postgresql::PostgresqlReporterArgs;
use crate::reporters::slack::SlackReporterArgs;
use crate::reporters::splunk::SplunkReporterArgs;

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    pub log_level: Option<String>,
    pub use_db_persistence: Option<bool>,
    pub monitor: Vec<MonitorArgs>,
    pub splunk: Option<SplunkReporterArgs>,
    pub slack: Option<SlackReporterArgs>,
    pub pagerduty: Option<PagerdutyReporterArgs>,
    pub postgresql: Option<PostgresqlReporterArgs>,
}

impl Config {
    fn new() -> Self {
        Config {
            log_level: None,
            use_db_persistence: None,
            monitor: Vec::new(),
            splunk: None,
            slack: None,
            pagerduty: None,
            postgresql: None,
        }
    }
    fn overlay(&mut self, other: &Self) {
        // could probably make this a lot cleaner with a macro
        // check each top-level config stanza, if set in other,
        // bring it in, otherwise do nothing
        if let Some(v) = other.log_level.clone() {
            self.log_level = Some(v)
        }
        if let Some(v) = other.use_db_persistence {
            self.use_db_persistence = Some(v)
        }
        if let Some(v) = other.splunk.clone() {
            self.splunk = Some(v)
        }
        if let Some(v) = other.slack.clone() {
            self.slack = Some(v)
        }
        if let Some(v) = other.pagerduty.clone() {
            self.pagerduty = Some(v)
        }
        if let Some(v) = other.postgresql.clone() {
            self.postgresql = Some(v)
        }
        // for monitors, we need to merge the Vecs based
        // on monitor name
        other.monitor.clone().into_iter().for_each(|m| {
            let pos = self.monitor.iter().position(|x| x.name == m.name);
            if let Some(pos) = pos {
                self.monitor.remove(pos);
                self.monitor.push(m);
            }
        })
    }
}

// Here we parse each non-None Config and merge them into one
// based on order precedence. We only need the first encountered config
// section of each type, no recursive merging of keys.
// This allows for lexicographic sorting of config files in the conf dir
// with the highest ordered files taking full precedence and any overlapping
// settings in lower ordered files being discarded.
pub fn parse_config(p: &str) -> Result<Config, ConfigError> {
    let mut config = Config::new();
    let path = Path::new(p);
    if path.is_dir() {
        print!("loading config directory {}", p);
        let mut count = 0;
        let mut co: Vec<Option<Config>> = Vec::new();
        let d = fs::read_dir(path).expect("unable to read config directory");
        d.for_each(|e| {
            if let Ok(r) = e {
                let input = &fs::read_to_string(r.path());
                match input {
                    Ok(v) => co.push(toml::from_str(v).ok()),
                    Err(_) => tracing::debug!("failed to parse file as toml"),
                }
            }
            count += 1;
        });
        co.into_iter().rev().for_each(|t| {
            if let Some(t) = t {
                config.overlay(&t)
            }
        })
    } else if path.is_file() {
        print!("loading config file {}", p);
        let input = &fs::read_to_string(path)?;
        config = toml::from_str(input)?;
    } else {
        return Err(ConfigError::InvalidPath(p.to_string()));
    }
    Ok(config)
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidPath(String),
    Io(io::Error),
    Toml(toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPath(e) => write!(f, "invalid config path provided ({})", e),
            ConfigError::Io(e) => write!(f, "IO error encountered ({})", e),
            ConfigError::Toml(e) => write!(f, "Toml error encountered ({})", e),
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        ConfigError::Io(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        ConfigError::Toml(value)
    }
}

// impl From<&str> for ConfigError {
//     fn from(value: &str) -> Self {
//         ConfigError {
//             message: value.to_string(),
//         }
//     }
// }

// impl From<std::string::String> for ConfigError {
//     fn from(value: String) -> Self {
//         ConfigError { message: value }
//     }
// }

// impl From<std::num::TryFromIntError> for ConfigError {
//     fn from(value: std::num::TryFromIntError) -> Self {
//         ConfigError {
//             message: format!(
//                 "unable to convert config value to specified integer type ({})",
//                 value
//             ),
//         }
//     }
// }
