use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 8563;

pub type Config = BTreeMap<String, Profile>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    pub schema: Option<String>,
    pub tls: Option<bool>,
    pub validate_certificate: Option<bool>,
}

impl Profile {
    pub fn to_dsn(&self) -> String {
        let port = self.port.unwrap_or(DEFAULT_PORT);
        let tls = self.tls.unwrap_or(true);
        let validate = self.validate_certificate.unwrap_or(true);
        let validate_int = if validate { 1 } else { 0 };

        let mut dsn = format!(
            "exasol://{}:{}@{}:{}",
            self.user, self.password, self.host, port
        );

        if let Some(ref schema) = self.schema {
            dsn.push('/');
            dsn.push_str(schema);
        }

        dsn.push_str(&format!(
            "?tls={}&validateservercertificate={}",
            tls, validate_int
        ));

        dsn
    }
}

pub fn docker_preset() -> Profile {
    Profile {
        host: "localhost".to_string(),
        port: Some(DEFAULT_PORT),
        user: "sys".to_string(),
        password: "exasol".to_string(),
        schema: None,
        tls: Some(true),
        validate_certificate: Some(false),
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("EXAPUMP_CONFIG") {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".exapump")
        .join("config.toml")
}

pub fn load_config() -> anyhow::Result<Config> {
    load_config_from(&config_path())
}

pub fn load_config_from(path: &std::path::Path) -> anyhow::Result<Config> {
    if !path.exists() {
        return Ok(Config::new());
    }
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &Config) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn validate_profile_name(name: &str) -> anyhow::Result<()> {
    let valid = !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        anyhow::bail!(
            "Invalid profile name '{}'. Must start with a letter or digit, \
             followed by letters, digits, underscores, or hyphens.",
            name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Write a TOML config file inside a temp directory and return the path.
    fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let config_dir = dir.join(".exapump");
        std::fs::create_dir_all(&config_dir).unwrap();
        let path = config_dir.join("config.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn config_file_parsed_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            dir.path(),
            r#"
[default]
host = "localhost"
port = 8563
user = "sys"
password = "exasol"
tls = true
validate_certificate = false
"#,
        );

        let config = load_config_from(&path).unwrap();
        assert!(config.contains_key("default"));
        let p = config.get("default").unwrap();
        assert_eq!(p.host, "localhost");
        assert_eq!(p.port, Some(8563));
        assert_eq!(p.user, "sys");
        assert_eq!(p.password, "exasol");
        assert_eq!(p.tls, Some(true));
        assert_eq!(p.validate_certificate, Some(false));
        assert_eq!(p.schema, None);
    }

    #[test]
    fn profile_builds_dsn() {
        let profile = Profile {
            host: "myhost".to_string(),
            port: Some(8563),
            user: "admin".to_string(),
            password: "secret".to_string(),
            schema: None,
            tls: Some(true),
            validate_certificate: Some(false),
        };

        let dsn = profile.to_dsn();
        assert_eq!(
            dsn,
            "exasol://admin:secret@myhost:8563?tls=true&validateservercertificate=0"
        );
    }

    #[test]
    fn profile_with_schema_builds_dsn() {
        let profile = Profile {
            host: "myhost".to_string(),
            port: Some(8563),
            user: "admin".to_string(),
            password: "secret".to_string(),
            schema: Some("my_schema".to_string()),
            tls: Some(true),
            validate_certificate: Some(true),
        };

        let dsn = profile.to_dsn();
        assert!(dsn.contains("/my_schema?"));
        assert_eq!(
            dsn,
            "exasol://admin:secret@myhost:8563/my_schema?tls=true&validateservercertificate=1"
        );
    }

    #[test]
    fn port_defaults_to_8563() {
        let profile = Profile {
            host: "myhost".to_string(),
            port: None,
            user: "u".to_string(),
            password: "p".to_string(),
            schema: None,
            tls: Some(true),
            validate_certificate: Some(true),
        };

        let dsn = profile.to_dsn();
        assert!(dsn.contains(":8563"));
    }

    #[test]
    fn tls_defaults_to_true() {
        let profile = Profile {
            host: "myhost".to_string(),
            port: Some(8563),
            user: "u".to_string(),
            password: "p".to_string(),
            schema: None,
            tls: None,
            validate_certificate: Some(true),
        };

        let dsn = profile.to_dsn();
        assert!(dsn.contains("tls=true"));
    }

    #[test]
    fn validate_certificate_defaults_to_true() {
        let profile = Profile {
            host: "myhost".to_string(),
            port: Some(8563),
            user: "u".to_string(),
            password: "p".to_string(),
            schema: None,
            tls: Some(true),
            validate_certificate: None,
        };

        let dsn = profile.to_dsn();
        assert!(dsn.contains("validateservercertificate=1"));
    }

    #[test]
    fn profile_dsn_maps_all_parameters() {
        let profile = Profile {
            host: "prod.example.com".to_string(),
            port: Some(9999),
            user: "admin".to_string(),
            password: "s3cret".to_string(),
            schema: Some("analytics".to_string()),
            tls: Some(false),
            validate_certificate: Some(false),
        };

        let dsn = profile.to_dsn();
        assert_eq!(
            dsn,
            "exasol://admin:s3cret@prod.example.com:9999/analytics?tls=false&validateservercertificate=0"
        );
    }

    #[test]
    fn valid_profile_names_accepted() {
        let valid_names = ["default", "my-docker", "prod_eu", "DB1"];
        for name in &valid_names {
            assert!(
                validate_profile_name(name).is_ok(),
                "Expected '{}' to be valid",
                name
            );
        }
    }

    #[test]
    fn invalid_profile_names_rejected() {
        let invalid_names = ["", "-start", "_start", "has space", "has.dot"];
        for name in &invalid_names {
            assert!(
                validate_profile_name(name).is_err(),
                "Expected '{}' to be invalid",
                name
            );
        }
    }
}
