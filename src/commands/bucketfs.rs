use std::path::Path;

use anyhow::Context;
use reqwest::Client;

use crate::cli::{BfsConnectionOverrides, BucketFsArgs, BucketfsCommands};
use crate::config::{self, BfsConnection, Config};

pub struct BucketFsClient {
    client: Client,
    base_url: String,
    bucket: String,
    write_password: Option<String>,
    read_password: Option<String>,
}

impl BucketFsClient {
    pub fn new(conn: BfsConnection) -> anyhow::Result<Self> {
        let mut builder = Client::builder();
        if !conn.validate_certificate {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder.build().context("Failed to build HTTP client")?;

        let scheme = if conn.tls { "https" } else { "http" };
        let base_url = format!("{scheme}://{}:{}", conn.host, conn.port);
        let read_password = conn.effective_read_password().map(str::to_string);

        Ok(Self {
            client,
            base_url,
            bucket: conn.bucket,
            write_password: conn.write_password,
            read_password,
        })
    }

    pub async fn list(&self, path: &str, recursive: bool) -> anyhow::Result<()> {
        let all_entries = self.list_bucket().await?;
        let prefix = path.trim_end_matches('/');

        let filtered: Vec<&str> = if prefix.is_empty() {
            all_entries.iter().map(|s| s.as_str()).collect()
        } else {
            all_entries
                .iter()
                .filter(|e| e.starts_with(prefix) && e.len() > prefix.len())
                .map(|e| {
                    // Strip the prefix and leading slash
                    let rest = &e[prefix.len()..];
                    rest.strip_prefix('/').unwrap_or(rest)
                })
                .collect()
        };

        if filtered.is_empty() && !prefix.is_empty() {
            anyhow::bail!("Path not found: {path}");
        }

        if recursive {
            for entry in &filtered {
                println!("{entry}");
            }
        } else {
            // Show only immediate children (unique first path segments)
            let mut seen = std::collections::BTreeSet::new();
            for entry in &filtered {
                let top = entry.split('/').next().unwrap_or(entry);
                if seen.insert(top) {
                    println!("{top}");
                }
            }
        }

        Ok(())
    }

    async fn list_bucket(&self) -> anyhow::Result<Vec<String>> {
        let url = format!("{}/{}/", self.base_url, self.bucket);

        let mut request = self.client.get(&url);
        if let Some(pw) = &self.read_password {
            request = request.basic_auth("r", Some(pw));
        }

        let response = request.send().await.map_err(|e| connect_error(&url, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "Authentication failed. Pass --bfs-read-password or --bfs-write-password, or set bfs_read_password or bfs_write_password in your profile."
            );
        }
        if !status.is_success() {
            anyhow::bail!("BucketFS returned HTTP {status}");
        }

        let body = response.text().await?;
        let entries: Vec<String> = body
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        Ok(entries)
    }

    pub async fn upload(&self, source: &str, destination: &str) -> anyhow::Result<()> {
        let write_password = self.write_password.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "A write password is required for write operations. Pass --bfs-write-password, or set bfs_write_password in your profile."
            )
        })?;

        let source_path = Path::new(source);
        if !source_path.exists() {
            anyhow::bail!("Source file not found: {source}");
        }

        let stripped_dest = strip_bfs_uri(destination);
        let dest = if stripped_dest.ends_with('/') {
            let filename = source_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Cannot determine filename from source: {source}"))?
                .to_string_lossy();
            format!("{stripped_dest}{filename}")
        } else {
            stripped_dest.to_string()
        };

        let url = format!("{}/{}/{dest}", self.base_url, self.bucket);
        let body = tokio::fs::read(source)
            .await
            .context("Failed to read source file")?;

        let response = self
            .client
            .put(&url)
            .basic_auth("w", Some(write_password))
            .body(body)
            .send()
            .await
            .map_err(|e| connect_error(&url, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "Authentication failed. Pass --bfs-write-password, or set bfs_write_password in your profile."
            );
        }
        if !status.is_success() {
            anyhow::bail!("BucketFS returned HTTP {status}");
        }

        eprintln!("Uploaded {source} to {dest}");
        Ok(())
    }

    pub async fn download(&self, source: &str, destination: &str) -> anyhow::Result<()> {
        let source = strip_bfs_uri(source);
        let url = format!("{}/{}/{source}", self.base_url, self.bucket);

        let mut request = self.client.get(&url);
        if let Some(pw) = &self.read_password {
            request = request.basic_auth("r", Some(pw));
        }

        let response = request.send().await.map_err(|e| connect_error(&url, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "Authentication failed. Pass --bfs-read-password or --bfs-write-password, or set bfs_read_password or bfs_write_password in your profile."
            );
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("File not found: {source}");
        }
        if !status.is_success() {
            anyhow::bail!("BucketFS returned HTTP {status}");
        }

        let dest_path = Path::new(destination);
        let final_path = if dest_path.is_dir() {
            let filename = Path::new(source).file_name().ok_or_else(|| {
                anyhow::anyhow!("Cannot determine filename from source: {source}")
            })?;
            dest_path.join(filename)
        } else {
            dest_path.to_path_buf()
        };

        let bytes = response.bytes().await?;
        tokio::fs::write(&final_path, &bytes)
            .await
            .context("Failed to write destination file")?;

        eprintln!("Downloaded {source} to {}", final_path.display());
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let write_password = self.write_password.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "A write password is required for write operations. Pass --bfs-write-password, or set bfs_write_password in your profile."
            )
        })?;

        let url = format!("{}/{}/{path}", self.base_url, self.bucket);

        let response = self
            .client
            .delete(&url)
            .basic_auth("w", Some(write_password))
            .send()
            .await
            .map_err(|e| connect_error(&url, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "Authentication failed. Pass --bfs-write-password, or set bfs_write_password in your profile."
            );
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("File not found: {path}");
        }
        if !status.is_success() {
            anyhow::bail!("BucketFS returned HTTP {status}");
        }

        eprintln!("Deleted {path}");
        Ok(())
    }
}

pub async fn run(args: BucketFsArgs) -> anyhow::Result<()> {
    let overrides = match &args.command {
        BucketfsCommands::Ls { conn, .. } => conn,
        BucketfsCommands::Cp { conn, .. } => conn,
        BucketfsCommands::Rm { conn, .. } => conn,
    };

    let config = config::load_config()?;
    let base = base_connection(&config, overrides)?;

    let mut conn = resolve_connection(&base, overrides);
    if let BucketfsCommands::Cp {
        source,
        destination,
        ..
    } = &args.command
    {
        if overrides.bfs_tls.is_none()
            && (source.starts_with("bfss://") || destination.starts_with("bfss://"))
        {
            conn.tls = true;
        }
    }
    let bfs = BucketFsClient::new(conn)?;

    match args.command {
        BucketfsCommands::Ls {
            path, recursive, ..
        } => bfs.list(&path.unwrap_or_default(), recursive).await,
        BucketfsCommands::Cp {
            source,
            destination,
            ..
        } => {
            if Path::new(&source).exists() {
                bfs.upload(&source, &destination).await
            } else {
                bfs.download(&source, &destination).await
            }
        }
        BucketfsCommands::Rm { path, .. } => bfs.delete(&path).await,
    }
}

/// Picks the base connection that the `--bfs-*` flags are then applied on top of.
///
/// The config file is a fallback value source, not a precondition: it is
/// consulted only when the flags leave a gap. Branch order below is the
/// precedence rule, so the first branch whose condition holds wins.
///
/// The function performs no I/O and reads no ambient state, so every branch is
/// reachable from a unit test.
fn base_connection(
    config: &Config,
    overrides: &BfsConnectionOverrides,
) -> anyhow::Result<BfsConnection> {
    if let Some(name) = &overrides.profile {
        let profile = config
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;
        return Ok(profile.resolve_bfs_connection());
    }

    let has_credential =
        overrides.bfs_write_password.is_some() || overrides.bfs_read_password.is_some();
    if let (Some(host), true) = (&overrides.bfs_host, has_credential) {
        return Ok(BfsConnection::with_defaults(host.clone()));
    }

    let profile_error = match config::find_default_profile(config) {
        Ok((_, profile)) => return Ok(profile.resolve_bfs_connection()),
        Err(error) => error,
    };

    match &overrides.bfs_host {
        Some(host) if config.is_empty() => Ok(BfsConnection::with_defaults(host.clone())),
        Some(_) => Err(profile_error),
        None => Err(profile_error.context(
            "No BucketFS connection configured. Pass --bfs-host together with \
             --bfs-write-password or --bfs-read-password, or create a profile with \
             `exapump profile add`.",
        )),
    }
}

fn resolve_connection(base: &BfsConnection, args: &BfsConnectionOverrides) -> BfsConnection {
    BfsConnection {
        host: args.bfs_host.clone().unwrap_or_else(|| base.host.clone()),
        port: args.bfs_port.unwrap_or(base.port),
        bucket: args
            .bfs_bucket
            .clone()
            .unwrap_or_else(|| base.bucket.clone()),
        write_password: args
            .bfs_write_password
            .clone()
            .or_else(|| base.write_password.clone()),
        read_password: args
            .bfs_read_password
            .clone()
            .or_else(|| base.read_password.clone()),
        tls: args.bfs_tls.unwrap_or(base.tls),
        validate_certificate: args
            .bfs_validate_certificate
            .unwrap_or(base.validate_certificate),
    }
}

fn extract_host_port(url: &str) -> &str {
    let after_scheme = match url.find("://") {
        Some(i) => &url[i + 3..],
        None => url,
    };
    match after_scheme.find('/') {
        Some(i) => &after_scheme[..i],
        None => after_scheme,
    }
}

fn connect_error(url: &str, err: reqwest::Error) -> anyhow::Error {
    if err.is_connect() {
        anyhow::anyhow!("BucketFS is not reachable at {}", extract_host_port(url))
    } else {
        anyhow::anyhow!("{err}")
    }
}

/// Normalises a BucketFS URI to a bare in-bucket path, stripping the scheme
/// and bucket prefix. Plain paths are returned unchanged.
pub fn strip_bfs_uri(path: &str) -> &str {
    let after_scheme = if let Some(rest) = path.strip_prefix("bfss://") {
        rest
    } else if let Some(rest) = path.strip_prefix("bfs://") {
        rest
    } else {
        return path;
    };
    match after_scheme.find('/') {
        Some(slash_pos) => &after_scheme[slash_pos + 1..],
        None => &after_scheme[after_scheme.len()..],
    }
}

#[cfg(test)]
mod tests {
    use super::{base_connection, resolve_connection, strip_bfs_uri};
    use crate::cli::BfsConnectionOverrides;
    use crate::config::{BfsConnection, Config, Profile};

    fn profile(host: &str) -> Profile {
        Profile {
            host: host.to_string(),
            port: None,
            user: "u".to_string(),
            password: "p".to_string(),
            schema: None,
            tls: None,
            validate_certificate: None,
            certificate_fingerprint: None,
            default: None,
            bfs_host: None,
            bfs_port: None,
            bfs_bucket: None,
            bfs_write_password: None,
            bfs_read_password: None,
            bfs_tls: None,
            bfs_validate_certificate: None,
        }
    }

    fn config_of(entries: Vec<(&str, Profile)>) -> Config {
        entries
            .into_iter()
            .map(|(name, p)| (name.to_string(), p))
            .collect()
    }

    /// A profile whose BucketFS fields all differ from the BucketFS defaults,
    /// so any value a test reads names the base it came from.
    fn distinctive_profile() -> Profile {
        let mut p = profile("profilehost");
        p.bfs_bucket = Some("wrongbucket".to_string());
        p.bfs_port = Some(9999);
        p.bfs_validate_certificate = Some(false);
        p.bfs_write_password = Some("profilepw".to_string());
        p
    }

    #[test]
    fn named_profile_stays_the_base_when_overrides_are_self_sufficient() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides {
            profile: Some("bfs".to_string()),
            bfs_host: Some("flaghost".to_string()),
            bfs_write_password: Some("flagpw".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();

        assert!(!base.validate_certificate);
        assert_eq!(base.bucket, "wrongbucket");
    }

    #[test]
    fn named_profile_that_is_not_in_the_config_is_an_error() {
        let config = config_of(vec![("bfs", profile("profilehost"))]);
        let overrides = BfsConnectionOverrides {
            profile: Some("ghost".to_string()),
            ..Default::default()
        };

        let err = base_connection(&config, &overrides).unwrap_err();

        assert!(format!("{err:#}").contains("Profile 'ghost' not found"));
    }

    #[test]
    fn host_with_write_password_selects_the_defaults_base() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            bfs_write_password: Some("flagpw".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();

        assert_eq!(base.host, "flaghost");
        assert_eq!(base.write_password, None);
    }

    #[test]
    fn host_with_read_password_selects_the_defaults_base() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            bfs_read_password: Some("flagpw".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();

        assert_eq!(base.bucket, "default");
        assert_eq!(base.port, 2581);
    }

    #[test]
    fn self_sufficient_overrides_ignore_profile_validate_certificate() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            bfs_write_password: Some("flagpw".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();

        assert_eq!(base.bucket, "default");
        assert_eq!(base.port, 2581);
        assert!(base.validate_certificate);
    }

    #[test]
    fn default_profile_is_the_base_when_no_override_is_given() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides::default();

        let base = base_connection(&config, &overrides).unwrap();

        assert_eq!(base.bucket, "wrongbucket");
        assert_eq!(base.host, "profilehost");
    }

    #[test]
    fn host_without_credential_inherits_the_default_profile() {
        let config = config_of(vec![("bfs", distinctive_profile())]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();

        assert_eq!(base.write_password, Some("profilepw".to_string()));
    }

    #[test]
    fn host_selects_the_defaults_base_when_the_config_holds_zero_profiles() {
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            ..Default::default()
        };

        let base = base_connection(&Config::new(), &overrides).unwrap();

        assert_eq!(base.host, "flaghost");
        assert_eq!(base.port, 2581);
        assert_eq!(base.bucket, "default");
        assert!(base.tls);
        assert!(base.validate_certificate);
        assert_eq!(base.write_password, None);
        assert_eq!(base.read_password, None);
    }

    #[test]
    fn host_propagates_the_ambiguous_default_profile_error() {
        let mut first = profile("host1");
        first.default = Some(true);
        let mut second = profile("host2");
        second.default = Some(true);
        let config = config_of(vec![("alpha", first), ("beta", second)]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            ..Default::default()
        };

        let err = base_connection(&config, &overrides).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("Multiple default profiles found"));
        assert!(message.contains("alpha"));
        assert!(message.contains("beta"));
        assert!(!message.contains("--bfs-host"));
    }

    #[test]
    fn host_propagates_the_missing_default_profile_error() {
        let config = config_of(vec![
            ("alpha", profile("host1")),
            ("beta", profile("host2")),
        ]);
        let overrides = BfsConnectionOverrides {
            bfs_host: Some("flaghost".to_string()),
            ..Default::default()
        };

        let err = base_connection(&config, &overrides).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("No default profile set"));
        assert!(message.contains("default = true"));
        assert!(!message.contains("--bfs-host"));
    }

    #[test]
    fn no_host_and_no_profile_names_both_remedies_and_keeps_the_cause() {
        let overrides = BfsConnectionOverrides::default();

        let err = base_connection(&Config::new(), &overrides).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("--bfs-host"));
        assert!(message.contains("exapump profile add"));
        assert!(message.contains("No profiles found in config"));
    }

    #[test]
    fn read_credential_falls_back_to_write_password_flag() {
        let base = BfsConnection::with_defaults("flaghost".to_string());
        let overrides = BfsConnectionOverrides {
            bfs_write_password: Some("wp".to_string()),
            ..Default::default()
        };

        let conn = resolve_connection(&base, &overrides);

        assert_eq!(conn.effective_read_password(), Some("wp"));
    }

    #[test]
    fn configured_read_password_outranks_write_password_flag() {
        let mut profile = profile("profilehost");
        profile.bfs_read_password = Some("rp".to_string());
        let base = profile.resolve_bfs_connection();
        let overrides = BfsConnectionOverrides {
            bfs_write_password: Some("wp".to_string()),
            ..Default::default()
        };

        let conn = resolve_connection(&base, &overrides);

        assert_eq!(conn.effective_read_password(), Some("rp"));
    }

    #[test]
    fn read_password_flag_outranks_the_base_read_password() {
        let mut profile = profile("profilehost");
        profile.bfs_read_password = Some("rp".to_string());
        let base = profile.resolve_bfs_connection();
        let overrides = BfsConnectionOverrides {
            bfs_read_password: Some("flagrp".to_string()),
            bfs_write_password: Some("wp".to_string()),
            ..Default::default()
        };

        let conn = resolve_connection(&base, &overrides);

        assert_eq!(conn.effective_read_password(), Some("flagrp"));
    }

    #[test]
    fn write_password_flag_replaces_the_profile_derived_read_credential() {
        let mut rotated = profile("profilehost");
        rotated.bfs_write_password = Some("old".to_string());
        let config = config_of(vec![("bfs", rotated)]);
        let overrides = BfsConnectionOverrides {
            bfs_write_password: Some("new".to_string()),
            ..Default::default()
        };

        let base = base_connection(&config, &overrides).unwrap();
        let conn = resolve_connection(&base, &overrides);

        assert_eq!(conn.effective_read_password(), Some("new"));
    }

    #[test]
    fn read_credential_stays_unset_without_any_password() {
        let base = BfsConnection::with_defaults("flaghost".to_string());
        let overrides = BfsConnectionOverrides::default();

        let conn = resolve_connection(&base, &overrides);

        assert_eq!(conn.effective_read_password(), None);
    }

    #[test]
    fn strips_scheme_and_bucket() {
        assert_eq!(strip_bfs_uri("bfs://default/path"), "path");
    }

    #[test]
    fn strips_scheme_and_bucket_nested_path() {
        assert_eq!(strip_bfs_uri("bfs://default/a/b"), "a/b");
    }

    #[test]
    fn plain_path_unchanged() {
        assert_eq!(strip_bfs_uri("path"), "path");
    }

    #[test]
    fn empty_remainder_after_bucket() {
        assert_eq!(strip_bfs_uri("bfs://default/"), "");
    }

    #[test]
    fn strips_bfss_scheme_and_bucket() {
        assert_eq!(strip_bfs_uri("bfss://default/path"), "path");
    }

    #[test]
    fn strips_bfss_scheme_and_bucket_nested_path() {
        assert_eq!(strip_bfs_uri("bfss://default/a/b"), "a/b");
    }

    #[test]
    fn bfss_empty_remainder_after_bucket() {
        assert_eq!(strip_bfs_uri("bfss://default/"), "");
    }

    #[test]
    fn bfss_no_slash_after_bucket() {
        assert_eq!(strip_bfs_uri("bfss://default"), "");
    }
}
