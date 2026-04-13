use clap::Args;

#[derive(Args)]
pub struct ConnectionArgs {
    /// Connection string (e.g., exasol://user:pwd@host:port)
    #[arg(short = 'd', long, env = "EXAPUMP_DSN")]
    pub dsn: Option<String>,

    /// Connection profile name from ~/.exapump/config.toml
    #[arg(short = 'p', long)]
    pub profile: Option<String>,

    /// Pin the TLS connection to the SHA-256 hex fingerprint of the server's DER certificate
    #[arg(long)]
    pub certificate_fingerprint: Option<String>,
}

impl ConnectionArgs {
    pub fn resolve_dsn(&self) -> anyhow::Result<String> {
        // Priority 1 & 2: --dsn flag or EXAPUMP_DSN env var (both handled by clap)
        if let Some(ref dsn) = self.dsn {
            return Ok(append_fingerprint(
                dsn,
                self.certificate_fingerprint.as_deref(),
            ));
        }

        // Priority 3: --profile <name>
        let config = crate::config::load_config()?;

        if let Some(ref name) = self.profile {
            return match config.get(name) {
                Some(profile) => Ok(self.profile_to_dsn(profile)),
                None => anyhow::bail!("Profile '{}' not found in config", name),
            };
        }

        // Priority 4: find default profile (auto-default for single profile, or `default = true`)
        let (_, profile) = crate::config::find_default_profile(&config)?;
        Ok(self.profile_to_dsn(profile))
    }

    fn profile_to_dsn(&self, profile: &crate::config::Profile) -> String {
        if self.certificate_fingerprint.is_some() {
            let mut overridden = profile.clone();
            overridden.certificate_fingerprint = self.certificate_fingerprint.clone();
            overridden.to_dsn()
        } else {
            profile.to_dsn()
        }
    }

    pub async fn connect(&self) -> anyhow::Result<exarrow_rs::Connection> {
        let dsn = self.resolve_dsn()?;
        let driver = exarrow_rs::Driver::new();
        let db = driver.open(&dsn)?;
        let conn = db.connect().await?;
        Ok(conn)
    }
}

fn append_fingerprint(dsn: &str, fingerprint: Option<&str>) -> String {
    match fingerprint {
        Some(fp) => {
            let separator = if dsn.contains('?') { '&' } else { '?' };
            format!("{}{}certificate_fingerprint={}", dsn, separator, fp)
        }
        None => dsn.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_fingerprint_adds_query_separator_when_absent() {
        let dsn = "exasol://user:pwd@host:8563";
        let result = append_fingerprint(dsn, Some("deadbeef"));
        assert_eq!(
            result,
            "exasol://user:pwd@host:8563?certificate_fingerprint=deadbeef"
        );
    }

    #[test]
    fn append_fingerprint_appends_when_query_present() {
        let dsn = "exasol://user:pwd@host:8563?tls=true&validateservercertificate=0";
        let result = append_fingerprint(dsn, Some("aabbcc112233"));
        assert_eq!(
            result,
            "exasol://user:pwd@host:8563?tls=true&validateservercertificate=0&certificate_fingerprint=aabbcc112233"
        );
    }

    #[test]
    fn append_fingerprint_returns_dsn_unchanged_when_none() {
        let dsn = "exasol://user:pwd@host:8563?tls=true";
        assert_eq!(append_fingerprint(dsn, None), dsn);
    }

    fn profile_with_fingerprint(fp: Option<&str>) -> crate::config::Profile {
        crate::config::Profile {
            host: "host".to_string(),
            port: Some(8563),
            user: "u".to_string(),
            password: "p".to_string(),
            schema: None,
            tls: Some(true),
            validate_certificate: Some(true),
            certificate_fingerprint: fp.map(str::to_string),
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

    #[test]
    fn flag_overrides_profile_fingerprint() {
        let args = ConnectionArgs {
            dsn: None,
            profile: None,
            certificate_fingerprint: Some("bbbbbb".to_string()),
        };
        let profile = profile_with_fingerprint(Some("aaaaaa"));
        let dsn = args.profile_to_dsn(&profile);
        assert!(dsn.contains("certificate_fingerprint=bbbbbb"), "got: {dsn}");
        assert!(
            !dsn.contains("certificate_fingerprint=aaaaaa"),
            "got: {dsn}"
        );
    }

    #[test]
    fn profile_fingerprint_flows_into_dsn_without_flag() {
        let args = ConnectionArgs {
            dsn: None,
            profile: None,
            certificate_fingerprint: None,
        };
        let profile = profile_with_fingerprint(Some("ccdd11"));
        let dsn = args.profile_to_dsn(&profile);
        assert!(dsn.contains("certificate_fingerprint=ccdd11"), "got: {dsn}");
    }

    #[test]
    fn no_fingerprint_anywhere_omits_param() {
        let args = ConnectionArgs {
            dsn: None,
            profile: None,
            certificate_fingerprint: None,
        };
        let profile = profile_with_fingerprint(None);
        let dsn = args.profile_to_dsn(&profile);
        assert!(!dsn.contains("certificate_fingerprint"), "got: {dsn}");
    }
}
