use std::fmt;

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config error: {}", self.message)
    }
}

impl From<&str> for ConfigError {
    fn from(value: &str) -> Self {
        ConfigError {
            message: value.to_string(),
        }
    }
}

impl From<std::num::TryFromIntError> for ConfigError {
    fn from(value: std::num::TryFromIntError) -> Self {
        ConfigError {
            message: format!(
                "unable to convert config value to specified integer type ({})",
                value
            ),
        }
    }
}

// Returns the specified string value from a toml table, or optionally the provided
// default value. If None is passed as the default, then a ConfigError will
// be returned if the requested value is not present in the toml table
pub fn get_str_or_else(
    c: &toml::Table,
    field: &str,
    default: Option<&str>,
) -> Result<String, ConfigError> {
    match c.get(field) {
        Some(n) => Ok(n
            .as_str()
            .ok_or("invalid type, expected String")?
            .to_string()),
        None => {
            if let Some(d) = default {
                Ok(d.to_string())
            } else {
                Err(ConfigError {
                    message: String::from("config item not found and no default provided"),
                })
            }
        }
    }
}

// Returns the specified integer value from a toml table, or optionally the provided
// default value. If None is passed as the default, then a ConfigError will
// be returned if the requested value is not present in the toml table
pub fn get_int_or_else(
    c: &toml::Table,
    field: &str,
    default: Option<i64>,
) -> Result<i64, ConfigError> {
    match c.get(field) {
        Some(n) => Ok(n.as_integer().ok_or("invalid type, expected Integer")?),
        None => {
            if let Some(d) = default {
                Ok(d)
            } else {
                Err(ConfigError {
                    message: String::from("config item not found and no default provided"),
                })
            }
        }
    }
}
