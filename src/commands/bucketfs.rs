use std::path::Path;

use anyhow::Context;
use reqwest::Client;

use crate::cli::{BfsConnectionOverrides, BucketFsArgs, BucketfsCommands};
use crate::config::{self, BfsConnection};

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

        Ok(Self {
            client,
            base_url,
            bucket: conn.bucket,
            write_password: conn.write_password,
            read_password: conn.read_password,
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
                "Authentication failed. Set bfs_read_password or bfs_write_password in your profile."
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
            anyhow::anyhow!("bfs_write_password is required for write operations")
        })?;

        let source_path = Path::new(source);
        if !source_path.exists() {
            anyhow::bail!("Source file not found: {source}");
        }

        let dest = if destination.ends_with('/') {
            let filename = source_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Cannot determine filename from source: {source}"))?
                .to_string_lossy();
            format!("{destination}{filename}")
        } else {
            destination.to_string()
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
            anyhow::bail!("Authentication failed. Set bfs_write_password in your profile.");
        }
        if !status.is_success() {
            anyhow::bail!("BucketFS returned HTTP {status}");
        }

        eprintln!("Uploaded {source} to {dest}");
        Ok(())
    }

    pub async fn download(&self, source: &str, destination: &str) -> anyhow::Result<()> {
        let url = format!("{}/{}/{source}", self.base_url, self.bucket);

        let mut request = self.client.get(&url);
        if let Some(pw) = &self.read_password {
            request = request.basic_auth("r", Some(pw));
        }

        let response = request.send().await.map_err(|e| connect_error(&url, e))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "Authentication failed. Set bfs_read_password or bfs_write_password in your profile."
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
            anyhow::anyhow!("bfs_write_password is required for write operations")
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
            anyhow::bail!("Authentication failed. Set bfs_write_password in your profile.");
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
    let profile = match &overrides.profile {
        Some(name) => config
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?
            .clone(),
        None => {
            let (_, p) = config::find_default_profile(&config)?;
            p.clone()
        }
    };

    let conn = resolve_connection(&profile.resolve_bfs_connection(), overrides);
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
