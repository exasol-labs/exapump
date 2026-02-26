use clap::Args;

#[derive(Args)]
pub struct ConnectionArgs {
    /// Connection string (e.g., exasol://user:pwd@host:port)
    #[arg(short = 'd', long, env = "EXAPUMP_DSN")]
    pub dsn: Option<String>,

    /// Connection profile name from ~/.exapump/config.toml
    #[arg(short = 'p', long)]
    pub profile: Option<String>,
}

impl ConnectionArgs {
    pub fn resolve_dsn(&self) -> anyhow::Result<String> {
        // Priority 1 & 2: --dsn flag or EXAPUMP_DSN env var (both handled by clap)
        if let Some(ref dsn) = self.dsn {
            return Ok(dsn.clone());
        }

        // Priority 3: --profile <name>
        let config = crate::config::load_config()?;

        if let Some(ref name) = self.profile {
            return match config.get(name) {
                Some(profile) => Ok(profile.to_dsn()),
                None => anyhow::bail!("Profile '{}' not found in config", name),
            };
        }

        // Priority 4: "default" profile
        if let Some(profile) = config.get("default") {
            return Ok(profile.to_dsn());
        }

        // Priority 5: nothing available
        anyhow::bail!(
            "No connection specified. Use --dsn, set EXAPUMP_DSN, or run:\n\n  \
             exapump profile add default\n"
        )
    }

    pub async fn connect(&self) -> anyhow::Result<exarrow_rs::Connection> {
        let dsn = self.resolve_dsn()?;
        let driver = exarrow_rs::Driver::new();
        let db = driver.open(&dsn)?;
        let conn = db.connect().await?;
        Ok(conn)
    }
}
