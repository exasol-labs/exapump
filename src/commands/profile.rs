use std::io::IsTerminal;

use crate::config::{self, Profile};

#[derive(clap::Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommands,
}

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
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
        /// SHA-256 hex fingerprint of the server's DER certificate (pins TLS to a specific cert)
        #[arg(long)]
        certificate_fingerprint: Option<String>,
        /// Mark this profile as the default connection
        #[arg(long)]
        default: bool,
        /// BucketFS host (defaults to profile host)
        #[arg(long)]
        bfs_host: Option<String>,
        /// BucketFS HTTPS port (default: 2581)
        #[arg(long)]
        bfs_port: Option<u16>,
        /// BucketFS bucket name (default: "default")
        #[arg(long)]
        bfs_bucket: Option<String>,
        /// BucketFS write password
        #[arg(long)]
        bfs_write_password: Option<String>,
        /// BucketFS read password
        #[arg(long)]
        bfs_read_password: Option<String>,
        /// BucketFS TLS enabled (defaults to profile tls)
        #[arg(long)]
        bfs_tls: Option<bool>,
        /// BucketFS certificate validation (defaults to profile validate_certificate)
        #[arg(long)]
        bfs_validate_certificate: Option<bool>,
    },
    /// Remove a profile
    Remove {
        name: String,
        /// Skip the interactive confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Interactively edit an existing profile (current values shown as defaults)
    Edit {
        name: String,
        /// Skip the optional BucketFS edit prompts
        #[arg(long)]
        no_bucketfs: bool,
    },
    /// Interactively create a profile (secure password prompt, no plaintext on command line)
    Init {
        /// Profile name (prompted if omitted)
        name: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        schema: Option<String>,
        /// SHA-256 hex fingerprint of the server's DER certificate (pins TLS to a specific cert)
        #[arg(long)]
        certificate_fingerprint: Option<String>,
        /// Mark this profile as the default connection (skips the confirm prompt)
        #[arg(long)]
        default: bool,
        /// Skip the optional BucketFS configuration prompts
        #[arg(long)]
        no_bucketfs: bool,
    },
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
    certificate_fingerprint: Option<String>,
    bfs_host: Option<String>,
    bfs_port: Option<u16>,
    bfs_bucket: Option<String>,
    bfs_write_password: Option<String>,
    bfs_read_password: Option<String>,
    bfs_tls: Option<bool>,
    bfs_validate_certificate: Option<bool>,
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
            certificate_fingerprint,
            default,
            bfs_host,
            bfs_port,
            bfs_bucket,
            bfs_write_password,
            bfs_read_password,
            bfs_tls,
            bfs_validate_certificate,
        } => {
            let overrides = ProfileOverrides {
                host,
                port,
                user,
                password,
                schema,
                tls,
                validate_certificate,
                certificate_fingerprint,
                bfs_host,
                bfs_port,
                bfs_bucket,
                bfs_write_password,
                bfs_read_password,
                bfs_tls,
                bfs_validate_certificate,
            };
            add(&name, overrides, default)
        }
        ProfileCommands::Remove { name, yes } => remove(&name, yes),
        ProfileCommands::Edit { name, no_bucketfs } => edit(&name, no_bucketfs),
        ProfileCommands::Init {
            name,
            host,
            port,
            user,
            schema,
            certificate_fingerprint,
            default,
            no_bucketfs,
        } => init(InitArgs {
            name,
            host,
            port,
            user,
            schema,
            certificate_fingerprint,
            default,
            no_bucketfs,
        }),
    }
}

struct InitArgs {
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    schema: Option<String>,
    certificate_fingerprint: Option<String>,
    default: bool,
    no_bucketfs: bool,
}

fn list() -> anyhow::Result<()> {
    let config = config::load_config()?;
    if config.is_empty() {
        println!("No profiles configured. Run `exapump profile add <name>` to get started.");
        return Ok(());
    }

    let default_name = config::find_default_profile(&config)
        .ok()
        .map(|(name, _)| name.clone());

    let mut names: Vec<&String> = config.keys().collect();
    names.sort();
    for name in names {
        if default_name.as_deref() == Some(name.as_str()) {
            println!("{} (default)", name);
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
            if let Some(ref fingerprint) = profile.certificate_fingerprint {
                println!("  certificate_fingerprint: {}", fingerprint);
            }
            println!("  default: {}", profile.default.unwrap_or(false));
            if let Some(ref bfs_host) = profile.bfs_host {
                println!("  bfs_host: {}", bfs_host);
            }
            if let Some(bfs_port) = profile.bfs_port {
                println!("  bfs_port: {}", bfs_port);
            }
            if let Some(ref bfs_bucket) = profile.bfs_bucket {
                println!("  bfs_bucket: {}", bfs_bucket);
            }
            if profile.bfs_write_password.is_some() {
                println!("  bfs_write_password: ****");
            }
            if profile.bfs_read_password.is_some() {
                println!("  bfs_read_password: ****");
            }
            if let Some(bfs_tls) = profile.bfs_tls {
                println!("  bfs_tls: {}", bfs_tls);
            }
            if let Some(bfs_validate_certificate) = profile.bfs_validate_certificate {
                println!("  bfs_validate_certificate: {}", bfs_validate_certificate);
            }
            Ok(())
        }
        None => anyhow::bail!("Profile '{}' not found", name),
    }
}

fn add(name: &str, overrides: ProfileOverrides, set_default: bool) -> anyhow::Result<()> {
    config::validate_profile_name(name)?;

    let mut config = config::load_config()?;
    if config.contains_key(name) {
        anyhow::bail!(
            "Profile '{}' already exists. Remove it first with `exapump profile remove {}`",
            name,
            name
        );
    }

    let auto_defaulted = !set_default && config.is_empty();
    let default_field = if set_default || auto_defaulted {
        Some(true)
    } else {
        None
    };

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
            certificate_fingerprint: overrides.certificate_fingerprint,
            default: default_field,
            bfs_host: overrides.bfs_host,
            bfs_port: overrides.bfs_port,
            bfs_bucket: overrides.bfs_bucket,
            bfs_write_password: overrides.bfs_write_password,
            bfs_read_password: overrides.bfs_read_password,
            bfs_tls: overrides.bfs_tls,
            bfs_validate_certificate: overrides.bfs_validate_certificate,
        }
    } else {
        // For non-default profiles, host, user, and password are required
        let host = overrides.host.ok_or_else(|| {
            anyhow::anyhow!("--host is required when adding a non-default profile")
        })?;
        let user = overrides.user.ok_or_else(|| {
            anyhow::anyhow!("--user is required when adding a non-default profile")
        })?;
        let password = match overrides.password {
            Some(p) => p,
            None => prompt_password_for(name)?,
        };
        Profile {
            host,
            port: overrides.port,
            user,
            password,
            schema: overrides.schema,
            tls: overrides.tls,
            validate_certificate: overrides.validate_certificate,
            certificate_fingerprint: overrides.certificate_fingerprint,
            default: default_field,
            bfs_host: overrides.bfs_host,
            bfs_port: overrides.bfs_port,
            bfs_bucket: overrides.bfs_bucket,
            bfs_write_password: overrides.bfs_write_password,
            bfs_read_password: overrides.bfs_read_password,
            bfs_tls: overrides.bfs_tls,
            bfs_validate_certificate: overrides.bfs_validate_certificate,
        }
    };

    if set_default {
        for (_, existing_profile) in config.iter_mut() {
            existing_profile.default = None;
        }
    }

    let default_suffix = if set_default || auto_defaulted {
        " (set as default)"
    } else {
        ""
    };
    println!(
        "Profile '{}' added (host={}, port={}, user={}, tls={}, validate_certificate={}){}",
        name,
        profile.host,
        profile.port.unwrap_or(config::DEFAULT_PORT),
        profile.user,
        profile.tls.unwrap_or(true),
        profile.validate_certificate.unwrap_or(true),
        default_suffix,
    );

    config.insert(name.to_string(), profile);
    config::save_config(&config)?;

    Ok(())
}

fn init(args: InitArgs) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "`exapump profile init` requires an interactive terminal. \
             Use `exapump profile add` with explicit flags for scripted setups."
        );
    }

    let existing = config::load_config()?;

    let name = match args.name {
        Some(n) => {
            config::validate_profile_name(&n)?;
            n
        }
        None => prompt_profile_name(&existing)?,
    };
    config::validate_profile_name(&name)?;
    if existing.contains_key(&name) {
        anyhow::bail!(
            "Profile '{}' already exists. Remove it first with `exapump profile remove {}`",
            name,
            name
        );
    }

    let host = match args.host {
        Some(h) => h,
        None => inquire_text("Host", None, true)?,
    };
    let port = match args.port {
        Some(p) => p,
        None => inquire_port()?,
    };
    let user = match args.user {
        Some(u) => u,
        None => inquire_text("User", None, true)?,
    };

    let password = prompt_new_password()?;

    let schema = match args.schema {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => {
            let s = inquire_text("Schema (optional)", Some(""), false)?;
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    };

    let tls = inquire_confirm("Enable TLS?", true)?;
    let validate_certificate = inquire_confirm("Validate server certificate?", true)?;

    let make_default = if args.default || existing.is_empty() {
        true
    } else {
        inquire_confirm("Set as the default profile?", false)?
    };

    let (
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        bfs_tls,
        bfs_validate_certificate,
    ) = if args.no_bucketfs {
        (None, None, None, None, None, None, None)
    } else {
        prompt_bucketfs()?
    };

    let profile = Profile {
        host,
        port: Some(port),
        user,
        password,
        schema,
        tls: Some(tls),
        validate_certificate: Some(validate_certificate),
        certificate_fingerprint: args.certificate_fingerprint,
        default: if make_default { Some(true) } else { None },
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        bfs_tls,
        bfs_validate_certificate,
    };

    let mut config = existing;
    if make_default {
        for (_, existing_profile) in config.iter_mut() {
            existing_profile.default = None;
        }
    }
    config.insert(name.clone(), profile);
    config::save_config(&config)?;

    let default_suffix = if make_default {
        " (set as default)"
    } else {
        ""
    };
    println!("Profile '{}' created{}", name, default_suffix);
    Ok(())
}

fn inquire_text(label: &str, default: Option<&str>, required: bool) -> anyhow::Result<String> {
    let prompt_text = format!("{}:", label);
    let mut prompt = inquire::Text::new(&prompt_text);
    if let Some(d) = default {
        prompt = prompt.with_default(d);
    }
    let value = prompt.prompt().map_err(map_inquire_err)?;
    if required && value.trim().is_empty() {
        anyhow::bail!("{} is required", label);
    }
    Ok(value)
}

fn inquire_confirm(label: &str, default: bool) -> anyhow::Result<bool> {
    inquire::Confirm::new(label)
        .with_default(default)
        .prompt()
        .map_err(map_inquire_err)
}

fn inquire_port() -> anyhow::Result<u16> {
    loop {
        let raw = inquire::Text::new("Port:")
            .with_default(&config::DEFAULT_PORT.to_string())
            .prompt()
            .map_err(map_inquire_err)?;
        match raw.trim().parse::<u16>() {
            Ok(p) if p > 0 => return Ok(p),
            _ => println!("  not a valid port — enter 1..65535"),
        }
    }
}

fn prompt_profile_name(existing: &config::Config) -> anyhow::Result<String> {
    loop {
        let default = if existing.is_empty() { "default" } else { "" };
        let mut p = inquire::Text::new("Profile name:");
        if !default.is_empty() {
            p = p.with_default(default);
        }
        let name = p.prompt().map_err(map_inquire_err)?;
        match config::validate_profile_name(&name) {
            Ok(_) => {
                if existing.contains_key(&name) {
                    println!("  '{}' already exists — choose another name", name);
                    continue;
                }
                return Ok(name);
            }
            Err(e) => println!("  {}", e),
        }
    }
}

fn prompt_new_password() -> anyhow::Result<String> {
    loop {
        let pw = rpassword::prompt_password("Password: ")?;
        if pw.is_empty() {
            println!("  password cannot be empty");
            continue;
        }
        let confirm = rpassword::prompt_password("Confirm password: ")?;
        if pw == confirm {
            return Ok(pw);
        }
        println!("  passwords did not match — try again");
    }
}

#[allow(clippy::type_complexity)]
fn prompt_bucketfs() -> anyhow::Result<(
    Option<String>,
    Option<u16>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
    Option<bool>,
)> {
    if !inquire_confirm(
        "Configure BucketFS? (needed for `exapump bucketfs` commands)",
        false,
    )? {
        return Ok((None, None, None, None, None, None, None));
    }

    let host_raw = inquire_text(
        "BucketFS host (blank = same as profile host)",
        Some(""),
        false,
    )?;
    let bfs_host = if host_raw.is_empty() {
        None
    } else {
        Some(host_raw)
    };

    let port_raw = inquire::Text::new("BucketFS port:")
        .with_default(&config::DEFAULT_BFS_PORT.to_string())
        .prompt()
        .map_err(map_inquire_err)?;
    let bfs_port = match port_raw.trim().parse::<u16>() {
        Ok(p) if p > 0 => Some(p),
        _ => anyhow::bail!("invalid BucketFS port: {}", port_raw),
    };

    let bucket = inquire::Text::new("Bucket name:")
        .with_default("default")
        .prompt()
        .map_err(map_inquire_err)?;
    let bfs_bucket = Some(bucket);

    let bfs_write_password = {
        let pw = rpassword::prompt_password("BucketFS write password (blank to skip): ")?;
        if pw.is_empty() {
            None
        } else {
            Some(pw)
        }
    };
    let bfs_read_password = {
        let pw = rpassword::prompt_password(
            "BucketFS read password (blank = same as write password): ",
        )?;
        if pw.is_empty() {
            None
        } else {
            Some(pw)
        }
    };

    Ok((
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        None,
        None,
    ))
}

fn map_inquire_err(e: inquire::InquireError) -> anyhow::Error {
    use inquire::InquireError;
    match e {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            anyhow::anyhow!("cancelled")
        }
        other => anyhow::anyhow!(other),
    }
}

fn prompt_password_for(name: &str) -> anyhow::Result<String> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "--password is required when adding a non-default profile. \
             Pass --password, set it in a TTY (exapump will prompt), \
             or use `exapump profile init` for the guided wizard."
        );
    }
    let prompt = format!("Password for profile '{}': ", name);
    let pw = rpassword::prompt_password(&prompt)?;
    if pw.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }
    Ok(pw)
}

fn remove(name: &str, yes: bool) -> anyhow::Result<()> {
    let mut config = config::load_config()?;
    if !config.contains_key(name) {
        anyhow::bail!("Profile '{}' not found", name);
    }

    if !yes {
        if !std::io::stdin().is_terminal() {
            anyhow::bail!(
                "Refusing to remove '{}' without confirmation. Pass --yes (-y) in scripted contexts.",
                name
            );
        }
        let confirmed = inquire::Confirm::new(&format!("Remove profile '{}'?", name))
            .with_default(false)
            .prompt()
            .map_err(map_inquire_err)?;
        if !confirmed {
            println!("Cancelled — profile '{}' not removed", name);
            return Ok(());
        }
    }

    config.remove(name);
    config::save_config(&config)?;
    println!("Profile '{}' removed", name);
    Ok(())
}

fn edit(name: &str, no_bucketfs: bool) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "`exapump profile edit` requires an interactive terminal. \
             Update config.toml directly for scripted edits."
        );
    }

    let mut config = config::load_config()?;
    let current = config
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?
        .clone();

    println!(
        "Editing profile '{}' — press Enter to keep current value.",
        name
    );

    let host = inquire_text("Host", Some(&current.host), true)?;
    let current_port_str = current.port.unwrap_or(config::DEFAULT_PORT).to_string();
    let port = loop {
        let raw = inquire::Text::new("Port:")
            .with_default(&current_port_str)
            .prompt()
            .map_err(map_inquire_err)?;
        match raw.trim().parse::<u16>() {
            Ok(p) if p > 0 => break p,
            _ => println!("  not a valid port — enter 1..65535"),
        }
    };
    let user = inquire_text("User", Some(&current.user), true)?;

    let change_password =
        inquire_confirm("Change password? (No keeps the existing password)", false)?;
    let password = if change_password {
        prompt_new_password()?
    } else {
        current.password.clone()
    };

    let current_schema = current.schema.clone().unwrap_or_default();
    let schema_raw = inquire_text("Schema (blank = none)", Some(&current_schema), false)?;
    let schema = if schema_raw.is_empty() {
        None
    } else {
        Some(schema_raw)
    };

    let tls = inquire_confirm("Enable TLS?", current.tls.unwrap_or(true))?;
    let validate_certificate = inquire_confirm(
        "Validate server certificate?",
        current.validate_certificate.unwrap_or(true),
    )?;

    let current_fp = current.certificate_fingerprint.clone().unwrap_or_default();
    let fp_raw = inquire_text(
        "Certificate fingerprint (blank = none)",
        Some(&current_fp),
        false,
    )?;
    let certificate_fingerprint = if fp_raw.is_empty() {
        None
    } else {
        Some(fp_raw)
    };

    let was_default = current.default.unwrap_or(false);
    let make_default = inquire_confirm("Set as the default profile?", was_default)?;

    let (
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        bfs_tls,
        bfs_validate_certificate,
    ) = if no_bucketfs {
        (
            current.bfs_host.clone(),
            current.bfs_port,
            current.bfs_bucket.clone(),
            current.bfs_write_password.clone(),
            current.bfs_read_password.clone(),
            current.bfs_tls,
            current.bfs_validate_certificate,
        )
    } else {
        edit_bucketfs(&current)?
    };

    let updated = Profile {
        host,
        port: Some(port),
        user,
        password,
        schema,
        tls: Some(tls),
        validate_certificate: Some(validate_certificate),
        certificate_fingerprint,
        default: if make_default { Some(true) } else { None },
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        bfs_tls,
        bfs_validate_certificate,
    };

    if make_default {
        for (existing_name, existing_profile) in config.iter_mut() {
            if existing_name != name {
                existing_profile.default = None;
            }
        }
    }

    config.insert(name.to_string(), updated);
    config::save_config(&config)?;

    let default_suffix = if make_default { " (default)" } else { "" };
    println!("Profile '{}' updated{}", name, default_suffix);
    Ok(())
}

#[allow(clippy::type_complexity)]
fn edit_bucketfs(
    current: &Profile,
) -> anyhow::Result<(
    Option<String>,
    Option<u16>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
    Option<bool>,
)> {
    let edit_bfs = inquire_confirm("Edit BucketFS settings?", false)?;
    if !edit_bfs {
        return Ok((
            current.bfs_host.clone(),
            current.bfs_port,
            current.bfs_bucket.clone(),
            current.bfs_write_password.clone(),
            current.bfs_read_password.clone(),
            current.bfs_tls,
            current.bfs_validate_certificate,
        ));
    }

    let host_default = current.bfs_host.clone().unwrap_or_default();
    let host_raw = inquire_text(
        "BucketFS host (blank = same as profile host)",
        Some(&host_default),
        false,
    )?;
    let bfs_host = if host_raw.is_empty() {
        None
    } else {
        Some(host_raw)
    };

    let port_default = current
        .bfs_port
        .unwrap_or(config::DEFAULT_BFS_PORT)
        .to_string();
    let port_raw = inquire::Text::new("BucketFS port:")
        .with_default(&port_default)
        .prompt()
        .map_err(map_inquire_err)?;
    let bfs_port = match port_raw.trim().parse::<u16>() {
        Ok(p) if p > 0 => Some(p),
        _ => anyhow::bail!("invalid BucketFS port: {}", port_raw),
    };

    let bucket_default = current
        .bfs_bucket
        .clone()
        .unwrap_or_else(|| "default".into());
    let bucket = inquire::Text::new("Bucket name:")
        .with_default(&bucket_default)
        .prompt()
        .map_err(map_inquire_err)?;
    let bfs_bucket = Some(bucket);

    let change_write =
        inquire_confirm("Change BucketFS write password? (No keeps existing)", false)?;
    let bfs_write_password = if change_write {
        let pw = rpassword::prompt_password("BucketFS write password (blank = clear): ")?;
        if pw.is_empty() {
            None
        } else {
            Some(pw)
        }
    } else {
        current.bfs_write_password.clone()
    };

    let change_read = inquire_confirm("Change BucketFS read password? (No keeps existing)", false)?;
    let bfs_read_password = if change_read {
        let pw = rpassword::prompt_password(
            "BucketFS read password (blank = fall back to write password): ",
        )?;
        if pw.is_empty() {
            None
        } else {
            Some(pw)
        }
    } else {
        current.bfs_read_password.clone()
    };

    Ok((
        bfs_host,
        bfs_port,
        bfs_bucket,
        bfs_write_password,
        bfs_read_password,
        current.bfs_tls,
        current.bfs_validate_certificate,
    ))
}
