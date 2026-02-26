use crate::config::{self, Profile};

#[derive(clap::Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommands,
}

#[derive(clap::Subcommand)]
pub enum ProfileCommands {
    /// List all profiles
    List,
    /// Show details of a profile
    Show { name: String },
    /// Add a new profile
    Add {
        name: String,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        schema: Option<String>,
        #[arg(long)]
        tls: Option<bool>,
        #[arg(long)]
        validate_certificate: Option<bool>,
    },
    /// Remove a profile
    Remove { name: String },
}

#[derive(Default)]
struct ProfileOverrides {
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    schema: Option<String>,
    tls: Option<bool>,
    validate_certificate: Option<bool>,
}

pub fn run(args: ProfileArgs) -> anyhow::Result<()> {
    match args.command {
        ProfileCommands::List => list(),
        ProfileCommands::Show { name } => show(&name),
        ProfileCommands::Add {
            name,
            host,
            port,
            user,
            password,
            schema,
            tls,
            validate_certificate,
        } => {
            let overrides = ProfileOverrides {
                host,
                port,
                user,
                password,
                schema,
                tls,
                validate_certificate,
            };
            add(&name, overrides)
        }
        ProfileCommands::Remove { name } => remove(&name),
    }
}

fn list() -> anyhow::Result<()> {
    let config = config::load_config()?;
    if config.is_empty() {
        println!("No profiles configured. Run `exapump profile add default` to get started.");
        return Ok(());
    }
    let mut names: Vec<&String> = config.keys().collect();
    names.sort();
    for name in names {
        if name == "default" {
            println!("{} *", name);
        } else {
            println!("{}", name);
        }
    }
    Ok(())
}

fn show(name: &str) -> anyhow::Result<()> {
    let config = config::load_config()?;
    match config.get(name) {
        Some(profile) => {
            println!("Profile '{}':", name);
            println!("  host: {}", profile.host);
            println!("  port: {}", profile.port.unwrap_or(config::DEFAULT_PORT));
            println!("  user: {}", profile.user);
            println!("  password: ****");
            if let Some(ref schema) = profile.schema {
                println!("  schema: {}", schema);
            }
            println!("  tls: {}", profile.tls.unwrap_or(true));
            println!(
                "  validate_certificate: {}",
                profile.validate_certificate.unwrap_or(true)
            );
            Ok(())
        }
        None => anyhow::bail!("Profile '{}' not found", name),
    }
}

fn add(name: &str, overrides: ProfileOverrides) -> anyhow::Result<()> {
    config::validate_profile_name(name)?;

    let mut config = config::load_config()?;
    if config.contains_key(name) {
        anyhow::bail!(
            "Profile '{}' already exists. Remove it first with `exapump profile remove {}`",
            name,
            name
        );
    }

    let profile = if name == "default" {
        // For "default", use Docker presets as base and override with any provided flags
        let preset = config::docker_preset();
        Profile {
            host: overrides.host.unwrap_or(preset.host),
            port: overrides.port.or(preset.port),
            user: overrides.user.unwrap_or(preset.user),
            password: overrides.password.unwrap_or(preset.password),
            schema: overrides.schema.or(preset.schema),
            tls: overrides.tls.or(preset.tls),
            validate_certificate: overrides
                .validate_certificate
                .or(preset.validate_certificate),
        }
    } else {
        // For non-default profiles, host, user, and password are required
        let host = overrides.host.ok_or_else(|| {
            anyhow::anyhow!("--host is required when adding a non-default profile")
        })?;
        let user = overrides.user.ok_or_else(|| {
            anyhow::anyhow!("--user is required when adding a non-default profile")
        })?;
        let password = overrides.password.ok_or_else(|| {
            anyhow::anyhow!("--password is required when adding a non-default profile")
        })?;
        Profile {
            host,
            port: overrides.port,
            user,
            password,
            schema: overrides.schema,
            tls: overrides.tls,
            validate_certificate: overrides.validate_certificate,
        }
    };

    println!(
        "Profile '{}' added (host={}, port={}, user={}, tls={}, validate_certificate={})",
        name,
        profile.host,
        profile.port.unwrap_or(config::DEFAULT_PORT),
        profile.user,
        profile.tls.unwrap_or(true),
        profile.validate_certificate.unwrap_or(true),
    );

    config.insert(name.to_string(), profile);
    config::save_config(&config)?;

    Ok(())
}

fn remove(name: &str) -> anyhow::Result<()> {
    let mut config = config::load_config()?;
    if config.remove(name).is_none() {
        anyhow::bail!("Profile '{}' not found", name);
    }
    config::save_config(&config)?;
    println!("Profile '{}' removed", name);
    Ok(())
}
