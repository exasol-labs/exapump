use clap::Args;

#[derive(Args)]
pub struct ConnectionArgs {
    /// Connection string (e.g., exasol://user:pwd@host:port)
    #[arg(short = 'd', long, env = "EXAPUMP_DSN")]
    pub dsn: String,
}

impl ConnectionArgs {
    pub async fn connect(&self) -> anyhow::Result<exarrow_rs::Connection> {
        let driver = exarrow_rs::Driver::new();
        let db = driver.open(&self.dsn)?;
        let conn = db.connect().await?;
        Ok(conn)
    }
}
