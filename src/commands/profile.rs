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

/// The seven BucketFS fields of a [`Profile`], carried by name between the
/// prompting flows and the profile they end up in. Naming them is what keeps a
/// field-order mistake a compile error instead of a silently swapped password:
/// the five `Option<String>`/`Option<bool>` fields are otherwise interchangeable
/// by type. `tls` and `validate_certificate` have no prompt of their own — the
/// flows either carry a profile's existing values forward or leave them unset.
#[derive(Debug, Clone, PartialEq)]
struct BucketFsSettings {
    host: Option<String>,
    port: Option<u16>,
    bucket: Option<String>,
    write_password: Option<String>,
    read_password: Option<String>,
    tls: Option<bool>,
    validate_certificate: Option<bool>,
}

impl BucketFsSettings {
    fn none() -> Self {
        Self {
            host: None,
            port: None,
            bucket: None,
            write_password: None,
            read_password: None,
            tls: None,
            validate_certificate: None,
        }
    }

    fn from_profile(profile: &Profile) -> Self {
        Self {
            host: profile.bfs_host.clone(),
            port: profile.bfs_port,
            bucket: profile.bfs_bucket.clone(),
            write_password: profile.bfs_write_password.clone(),
            read_password: profile.bfs_read_password.clone(),
            tls: profile.bfs_tls,
            validate_certificate: profile.bfs_validate_certificate,
        }
    }
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
    let rows = profile_rows(config.get(name), name)?;
    println!("Profile '{}':", name);
    for (label, value) in rows {
        println!("  {}: {}", label, value);
    }
    Ok(())
}

/// Maps a profile lookup result to the ordered label/value rows `show` prints.
/// Masks `password`, `bfs_write_password` and `bfs_read_password` as `****`
/// rather than printing their real values. `profile` is the result of looking
/// up `name` in the config; a miss becomes this function's "not found" error,
/// so `show` need only load the config and print what comes back.
fn profile_rows(
    profile: Option<&Profile>,
    name: &str,
) -> anyhow::Result<Vec<(&'static str, String)>> {
    let profile = profile.ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

    let mut rows = vec![
        ("host", profile.host.clone()),
        (
            "port",
            profile.port.unwrap_or(config::DEFAULT_PORT).to_string(),
        ),
        ("user", profile.user.clone()),
        ("password", "****".to_string()),
    ];
    if let Some(ref schema) = profile.schema {
        rows.push(("schema", schema.clone()));
    }
    rows.push(("tls", profile.tls.unwrap_or(true).to_string()));
    rows.push((
        "validate_certificate",
        profile.validate_certificate.unwrap_or(true).to_string(),
    ));
    if let Some(ref fingerprint) = profile.certificate_fingerprint {
        rows.push(("certificate_fingerprint", fingerprint.clone()));
    }
    rows.push(("default", profile.default.unwrap_or(false).to_string()));
    if let Some(ref bfs_host) = profile.bfs_host {
        rows.push(("bfs_host", bfs_host.clone()));
    }
    if let Some(bfs_port) = profile.bfs_port {
        rows.push(("bfs_port", bfs_port.to_string()));
    }
    if let Some(ref bfs_bucket) = profile.bfs_bucket {
        rows.push(("bfs_bucket", bfs_bucket.clone()));
    }
    if profile.bfs_write_password.is_some() {
        rows.push(("bfs_write_password", "****".to_string()));
    }
    if profile.bfs_read_password.is_some() {
        rows.push(("bfs_read_password", "****".to_string()));
    }
    if let Some(bfs_tls) = profile.bfs_tls {
        rows.push(("bfs_tls", bfs_tls.to_string()));
    }
    if let Some(bfs_validate_certificate) = profile.bfs_validate_certificate {
        rows.push((
            "bfs_validate_certificate",
            bfs_validate_certificate.to_string(),
        ));
    }

    Ok(rows)
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
        for existing_profile in config.values_mut() {
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

/// The prompting surface the `init` and `edit` flows depend on, inverted so
/// those flows carry no `inquire`/`rpassword` call of their own and run whole
/// under test. `text` is a provided method on purpose: it owns label rendering
/// and the `required` empty-check, so no implementation can diverge from
/// another on either. Implementations supply only the raw reads.
trait ProfilePrompter {
    fn ask_text(&mut self, prompt: &str, default: Option<&str>) -> anyhow::Result<String>;

    fn confirm(&mut self, label: &str, default: bool) -> anyhow::Result<bool>;

    fn password(&mut self, prompt: &str) -> anyhow::Result<String>;

    fn notice(&mut self, message: &str);

    fn text(&mut self, label: &str, default: Option<&str>) -> anyhow::Result<String> {
        self.ask_text(&format!("{}:", label), default)
    }

    fn required_text(&mut self, label: &str, default: Option<&str>) -> anyhow::Result<String> {
        let value = self.text(label, default)?;
        if value.trim().is_empty() {
            anyhow::bail!("{} is required", label);
        }
        Ok(value)
    }
}

struct TerminalPrompter;

impl ProfilePrompter for TerminalPrompter {
    fn ask_text(&mut self, prompt: &str, default: Option<&str>) -> anyhow::Result<String> {
        let mut text = inquire::Text::new(prompt);
        if let Some(d) = default {
            text = text.with_default(d);
        }
        text.prompt().map_err(map_inquire_err)
    }

    fn confirm(&mut self, label: &str, default: bool) -> anyhow::Result<bool> {
        inquire::Confirm::new(label)
            .with_default(default)
            .prompt()
            .map_err(map_inquire_err)
    }

    fn password(&mut self, prompt: &str) -> anyhow::Result<String> {
        Ok(rpassword::prompt_password(prompt)?)
    }

    fn notice(&mut self, message: &str) {
        println!("{}", message);
    }
}

fn init(args: InitArgs) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "`exapump profile init` requires an interactive terminal. \
             Use `exapump profile add` with explicit flags for scripted setups."
        );
    }

    let mut config = config::load_config()?;
    let (name, profile) = init_profile(&mut TerminalPrompter, args, &config)?;

    let make_default = profile.default.unwrap_or(false);
    if make_default {
        clear_other_defaults(&mut config, None);
    }
    let default_suffix = if make_default {
        " (set as default)"
    } else {
        ""
    };
    config.insert(name.clone(), profile);
    config::save_config(&config)?;

    println!("Profile '{}' created{}", name, default_suffix);
    Ok(())
}

/// Collects a new profile through `prompter`, without reading or writing the
/// config file. `existing` is only inspected — for the duplicate-name check and
/// for the "first profile becomes the default" rule — so the caller keeps sole
/// ownership of persisting the result.
fn init_profile(
    prompter: &mut dyn ProfilePrompter,
    args: InitArgs,
    existing: &config::Config,
) -> anyhow::Result<(String, Profile)> {
    let name = match args.name {
        Some(n) => {
            config::validate_profile_name(&n)?;
            n
        }
        None => prompt_profile_name(prompter, existing)?,
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
        None => prompter.required_text("Host", None)?,
    };
    let port = match args.port {
        Some(p) => p,
        None => inquire_port(prompter, &config::DEFAULT_PORT.to_string())?,
    };
    let user = match args.user {
        Some(u) => u,
        None => prompter.required_text("User", None)?,
    };

    let password = prompt_new_password(prompter)?;

    let schema = match args.schema {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => blank_to_none(prompter.text("Schema (optional)", Some(""))?),
    };

    let tls = prompter.confirm("Enable TLS?", true)?;
    let validate_certificate = prompter.confirm("Validate server certificate?", true)?;

    let make_default = if args.default || existing.is_empty() {
        true
    } else {
        prompter.confirm("Set as the default profile?", false)?
    };

    let bucketfs = if args.no_bucketfs {
        BucketFsSettings::none()
    } else {
        prompt_bucketfs(prompter)?
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
        default: make_default.then_some(true),
        bfs_host: bucketfs.host,
        bfs_port: bucketfs.port,
        bfs_bucket: bucketfs.bucket,
        bfs_write_password: bucketfs.write_password,
        bfs_read_password: bucketfs.read_password,
        bfs_tls: bucketfs.tls,
        bfs_validate_certificate: bucketfs.validate_certificate,
    };

    Ok((name, profile))
}

/// Drops the `default` flag from every profile except `except`, so at most one
/// profile in the config claims it. `None` clears the flag everywhere.
fn clear_other_defaults(config: &mut config::Config, except: Option<&str>) {
    for (existing_name, existing_profile) in config.iter_mut() {
        if Some(existing_name.as_str()) != except {
            existing_profile.default = None;
        }
    }
}

fn parse_port(raw: &str) -> Option<u16> {
    match raw.trim().parse::<u16>() {
        Ok(p) if p > 0 => Some(p),
        _ => None,
    }
}

fn blank_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn inquire_port(prompter: &mut dyn ProfilePrompter, default: &str) -> anyhow::Result<u16> {
    loop {
        let raw = prompter.text("Port", Some(default))?;
        match parse_port(&raw) {
            Some(p) => return Ok(p),
            None => prompter.notice("  not a valid port — enter 1..65535"),
        }
    }
}

fn prompt_profile_name(
    prompter: &mut dyn ProfilePrompter,
    existing: &config::Config,
) -> anyhow::Result<String> {
    let default = if existing.is_empty() {
        Some("default")
    } else {
        None
    };
    loop {
        let name = prompter.text("Profile name", default)?;
        match config::validate_profile_name(&name) {
            Ok(_) => {
                if existing.contains_key(&name) {
                    prompter.notice(&format!(
                        "  '{}' already exists — choose another name",
                        name
                    ));
                    continue;
                }
                return Ok(name);
            }
            Err(e) => prompter.notice(&format!("  {}", e)),
        }
    }
}

fn prompt_new_password(prompter: &mut dyn ProfilePrompter) -> anyhow::Result<String> {
    loop {
        let pw = prompter.password("Password: ")?;
        if pw.is_empty() {
            prompter.notice("  password cannot be empty");
            continue;
        }
        let confirm = prompter.password("Confirm password: ")?;
        if pw == confirm {
            return Ok(pw);
        }
        prompter.notice("  passwords did not match — try again");
    }
}

fn prompt_bucketfs(prompter: &mut dyn ProfilePrompter) -> anyhow::Result<BucketFsSettings> {
    if !prompter.confirm(
        "Configure BucketFS? (needed for `exapump bucketfs` commands)",
        false,
    )? {
        return Ok(BucketFsSettings::none());
    }

    let host =
        blank_to_none(prompter.text("BucketFS host (blank = same as profile host)", Some(""))?);

    let port_raw = prompter.text("BucketFS port", Some(&config::DEFAULT_BFS_PORT.to_string()))?;
    let port = Some(
        parse_port(&port_raw)
            .ok_or_else(|| anyhow::anyhow!("invalid BucketFS port: {}", port_raw))?,
    );

    let bucket = Some(prompter.text("Bucket name", Some("default"))?);

    let write_password =
        blank_to_none(prompter.password("BucketFS write password (blank to skip): ")?);
    let read_password = blank_to_none(
        prompter.password("BucketFS read password (blank = same as write password): ")?,
    );

    Ok(BucketFsSettings {
        host,
        port,
        bucket,
        write_password,
        read_password,
        tls: None,
        validate_certificate: None,
    })
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

    let updated = edit_profile(&mut TerminalPrompter, &current, no_bucketfs)?;

    let make_default = updated.default.unwrap_or(false);
    if make_default {
        clear_other_defaults(&mut config, Some(name));
    }

    config.insert(name.to_string(), updated);
    config::save_config(&config)?;

    let default_suffix = if make_default { " (default)" } else { "" };
    println!("Profile '{}' updated{}", name, default_suffix);
    Ok(())
}

/// Re-collects every field of `current` through `prompter`, offering the
/// present value as each prompt's default so an empty answer keeps it. Returns
/// the replacement profile without touching the config file, leaving the caller
/// to persist it and to reconcile the `default` flag across profiles.
fn edit_profile(
    prompter: &mut dyn ProfilePrompter,
    current: &Profile,
    no_bucketfs: bool,
) -> anyhow::Result<Profile> {
    let host = prompter.required_text("Host", Some(&current.host))?;
    let current_port_str = current.port.unwrap_or(config::DEFAULT_PORT).to_string();
    let port = inquire_port(prompter, &current_port_str)?;
    let user = prompter.required_text("User", Some(&current.user))?;

    let change_password =
        prompter.confirm("Change password? (No keeps the existing password)", false)?;
    let password = if change_password {
        prompt_new_password(prompter)?
    } else {
        current.password.clone()
    };

    let current_schema = current.schema.clone().unwrap_or_default();
    let schema = blank_to_none(prompter.text("Schema (blank = none)", Some(&current_schema))?);

    let tls = prompter.confirm("Enable TLS?", current.tls.unwrap_or(true))?;
    let validate_certificate = prompter.confirm(
        "Validate server certificate?",
        current.validate_certificate.unwrap_or(true),
    )?;

    let current_fp = current.certificate_fingerprint.clone().unwrap_or_default();
    let certificate_fingerprint =
        blank_to_none(prompter.text("Certificate fingerprint (blank = none)", Some(&current_fp))?);

    let was_default = current.default.unwrap_or(false);
    let make_default = prompter.confirm("Set as the default profile?", was_default)?;

    let bucketfs = if no_bucketfs {
        BucketFsSettings::from_profile(current)
    } else {
        edit_bucketfs(prompter, current)?
    };

    Ok(Profile {
        host,
        port: Some(port),
        user,
        password,
        schema,
        tls: Some(tls),
        validate_certificate: Some(validate_certificate),
        certificate_fingerprint,
        default: make_default.then_some(true),
        bfs_host: bucketfs.host,
        bfs_port: bucketfs.port,
        bfs_bucket: bucketfs.bucket,
        bfs_write_password: bucketfs.write_password,
        bfs_read_password: bucketfs.read_password,
        bfs_tls: bucketfs.tls,
        bfs_validate_certificate: bucketfs.validate_certificate,
    })
}

fn edit_bucketfs(
    prompter: &mut dyn ProfilePrompter,
    current: &Profile,
) -> anyhow::Result<BucketFsSettings> {
    let edit_bfs = prompter.confirm("Edit BucketFS settings?", false)?;
    if !edit_bfs {
        return Ok(BucketFsSettings::from_profile(current));
    }

    let host_default = current.bfs_host.clone().unwrap_or_default();
    let host = blank_to_none(prompter.text(
        "BucketFS host (blank = same as profile host)",
        Some(&host_default),
    )?);

    let port_default = current
        .bfs_port
        .unwrap_or(config::DEFAULT_BFS_PORT)
        .to_string();
    let port_raw = prompter.text("BucketFS port", Some(&port_default))?;
    let port = Some(
        parse_port(&port_raw)
            .ok_or_else(|| anyhow::anyhow!("invalid BucketFS port: {}", port_raw))?,
    );

    let bucket_default = current
        .bfs_bucket
        .clone()
        .unwrap_or_else(|| "default".into());
    let bucket = Some(prompter.text("Bucket name", Some(&bucket_default))?);

    let change_write =
        prompter.confirm("Change BucketFS write password? (No keeps existing)", false)?;
    let write_password = if change_write {
        blank_to_none(prompter.password("BucketFS write password (blank = clear): ")?)
    } else {
        current.bfs_write_password.clone()
    };

    let change_read =
        prompter.confirm("Change BucketFS read password? (No keeps existing)", false)?;
    let read_password = if change_read {
        blank_to_none(
            prompter.password("BucketFS read password (blank = fall back to write password): ")?,
        )
    } else {
        current.bfs_read_password.clone()
    };

    Ok(BucketFsSettings {
        host,
        port,
        bucket,
        write_password,
        read_password,
        tls: current.bfs_tls,
        validate_certificate: current.bfs_validate_certificate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_profile() -> Profile {
        Profile {
            host: "db.example.com".to_string(),
            port: Some(1234),
            user: "admin".to_string(),
            password: "secret".to_string(),
            schema: Some("analytics".to_string()),
            tls: Some(false),
            validate_certificate: Some(false),
            certificate_fingerprint: Some("ab:cd:ef".to_string()),
            default: Some(true),
            bfs_host: Some("bfs.example.com".to_string()),
            bfs_port: Some(2581),
            bfs_bucket: Some("mybucket".to_string()),
            bfs_write_password: Some("writepw".to_string()),
            bfs_read_password: Some("readpw".to_string()),
            bfs_tls: Some(true),
            bfs_validate_certificate: Some(true),
        }
    }

    fn minimal_profile() -> Profile {
        Profile {
            host: "localhost".to_string(),
            port: None,
            user: "sys".to_string(),
            password: "exasol".to_string(),
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

    #[test]
    fn profile_rows_masks_passwords_and_lists_every_field_for_a_fully_populated_profile() {
        let profile = full_profile();

        let rows = profile_rows(Some(&profile), "demo").unwrap();

        assert_eq!(
            rows,
            vec![
                ("host", "db.example.com".to_string()),
                ("port", "1234".to_string()),
                ("user", "admin".to_string()),
                ("password", "****".to_string()),
                ("schema", "analytics".to_string()),
                ("tls", "false".to_string()),
                ("validate_certificate", "false".to_string()),
                ("certificate_fingerprint", "ab:cd:ef".to_string()),
                ("default", "true".to_string()),
                ("bfs_host", "bfs.example.com".to_string()),
                ("bfs_port", "2581".to_string()),
                ("bfs_bucket", "mybucket".to_string()),
                ("bfs_write_password", "****".to_string()),
                ("bfs_read_password", "****".to_string()),
                ("bfs_tls", "true".to_string()),
                ("bfs_validate_certificate", "true".to_string()),
            ]
        );
    }

    #[test]
    fn profile_rows_applies_defaults_and_omits_unset_optional_fields_for_a_minimal_profile() {
        let profile = minimal_profile();

        let rows = profile_rows(Some(&profile), "default").unwrap();

        assert_eq!(
            rows,
            vec![
                ("host", "localhost".to_string()),
                ("port", config::DEFAULT_PORT.to_string()),
                ("user", "sys".to_string()),
                ("password", "****".to_string()),
                ("tls", "true".to_string()),
                ("validate_certificate", "true".to_string()),
                ("default", "false".to_string()),
            ]
        );
    }

    #[test]
    fn profile_rows_errors_when_the_profile_is_missing() {
        let err = profile_rows(None, "ghost").unwrap_err();

        assert_eq!(err.to_string(), "Profile 'ghost' not found");
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Asked {
        Text(String, Option<String>),
        Confirm(String, bool),
        Password(String),
        Notice(String),
    }

    fn asked_text(prompt: &str, default: Option<&str>) -> Asked {
        Asked::Text(prompt.to_string(), default.map(str::to_string))
    }

    fn asked_confirm(label: &str, default: bool) -> Asked {
        Asked::Confirm(label.to_string(), default)
    }

    fn asked_password(prompt: &str) -> Asked {
        Asked::Password(prompt.to_string())
    }

    fn asked_notice(message: &str) -> Asked {
        Asked::Notice(message.to_string())
    }

    /// The prompt sequence [`prompt_bucketfs`] opens with: the configure
    /// confirmation and its three text prompts. Every test that reaches part of
    /// this sequence builds its expectation from here, so each prompt string is
    /// written once and a wording change fails every affected test at once.
    fn prompt_bucketfs_prompts(port_default: &str) -> Vec<Asked> {
        vec![
            asked_confirm(
                "Configure BucketFS? (needed for `exapump bucketfs` commands)",
                false,
            ),
            asked_text("BucketFS host (blank = same as profile host):", Some("")),
            asked_text("BucketFS port:", Some(port_default)),
            asked_text("Bucket name:", Some("default")),
        ]
    }

    /// The prompt sequence [`edit_bucketfs`] opens with: the edit confirmation
    /// and its three text prompts, each offering the current profile's value as
    /// its default. Serves the same single-source purpose as
    /// [`prompt_bucketfs_prompts`]; the password prompts that follow differ per
    /// test and stay in the test that asserts them.
    fn edit_bucketfs_prompts(
        host_default: &str,
        port_default: &str,
        bucket_default: &str,
    ) -> Vec<Asked> {
        vec![
            asked_confirm("Edit BucketFS settings?", false),
            asked_text(
                "BucketFS host (blank = same as profile host):",
                Some(host_default),
            ),
            asked_text("BucketFS port:", Some(port_default)),
            asked_text("Bucket name:", Some(bucket_default)),
        ]
    }

    #[derive(Default)]
    struct ScriptedPrompter {
        texts: std::collections::VecDeque<Result<String, String>>,
        confirms: std::collections::VecDeque<Result<bool, String>>,
        passwords: std::collections::VecDeque<String>,
        asked: Vec<Asked>,
    }

    impl ScriptedPrompter {
        fn new() -> Self {
            Self::default()
        }

        fn texts(mut self, answers: &[&str]) -> Self {
            self.texts
                .extend(answers.iter().map(|a| Ok((*a).to_string())));
            self
        }

        fn confirms(mut self, answers: &[bool]) -> Self {
            self.confirms.extend(answers.iter().copied().map(Ok));
            self
        }

        fn passwords(mut self, answers: &[&str]) -> Self {
            self.passwords = answers.iter().map(|a| (*a).to_string()).collect();
            self
        }

        /// Queues an error for the next `text`/`ask_text` call, so a test can
        /// reach the `?`-propagation branch inside the caller that awaited it.
        fn text_error(mut self, message: &str) -> Self {
            self.texts.push_back(Err(message.to_string()));
            self
        }

        /// Queues an error for the next `confirm` call, so a test can reach
        /// the `?`-propagation branch inside the caller that awaited it.
        fn confirm_error(mut self, message: &str) -> Self {
            self.confirms.push_back(Err(message.to_string()));
            self
        }
    }

    impl ProfilePrompter for ScriptedPrompter {
        fn ask_text(&mut self, prompt: &str, default: Option<&str>) -> anyhow::Result<String> {
            self.asked
                .push(Asked::Text(prompt.to_string(), default.map(str::to_string)));
            self.texts
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted text answer for {:?}", prompt))
                .map_err(|e| anyhow::anyhow!(e))
        }

        fn confirm(&mut self, label: &str, default: bool) -> anyhow::Result<bool> {
            self.asked.push(Asked::Confirm(label.to_string(), default));
            self.confirms
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted confirm answer for {:?}", label))
                .map_err(|e| anyhow::anyhow!(e))
        }

        fn password(&mut self, prompt: &str) -> anyhow::Result<String> {
            self.asked.push(Asked::Password(prompt.to_string()));
            Ok(self
                .passwords
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted password answer for {:?}", prompt)))
        }

        fn notice(&mut self, message: &str) {
            self.asked.push(Asked::Notice(message.to_string()));
        }
    }

    #[test]
    fn prompt_bucketfs_fails_hard_on_an_invalid_port_without_re_prompting() {
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .texts(&["", "not-a-port"]);

        let err = prompt_bucketfs(&mut prompter).unwrap_err();

        assert_eq!(err.to_string(), "invalid BucketFS port: not-a-port");
        assert_eq!(prompter.asked, prompt_bucketfs_prompts("2581")[..3]);
    }

    #[test]
    fn prompt_bucketfs_propagates_an_error_from_the_configure_confirmation() {
        let mut prompter = ScriptedPrompter::new().confirm_error("cancelled");

        let err = prompt_bucketfs(&mut prompter).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn prompt_bucketfs_propagates_an_error_from_the_host_prompt() {
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .text_error("cancelled");

        let err = prompt_bucketfs(&mut prompter).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn prompt_bucketfs_propagates_an_error_from_the_port_prompt() {
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .texts(&[""])
            .text_error("cancelled");

        let err = prompt_bucketfs(&mut prompter).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn required_text_rejects_a_blank_answer_when_the_field_is_required() {
        let mut prompter = ScriptedPrompter::new().texts(&["   "]);

        let err = prompter.required_text("Host", None).unwrap_err();

        assert_eq!(err.to_string(), "Host is required");
        assert_eq!(prompter.asked, vec![asked_text("Host:", None)]);
    }

    #[test]
    fn text_renders_the_label_with_a_trailing_colon_and_forwards_the_default() {
        let mut prompter = ScriptedPrompter::new().texts(&["answer"]);

        let value = prompter.text("Host", Some("db.example.com")).unwrap();

        assert_eq!(value, "answer");
        assert_eq!(
            prompter.asked,
            vec![asked_text("Host:", Some("db.example.com"))]
        );
    }

    #[test]
    fn text_accepts_a_blank_answer_when_the_field_is_optional() {
        let mut prompter = ScriptedPrompter::new().texts(&[""]);

        let value = prompter.text("Schema (optional)", Some("")).unwrap();

        assert_eq!(value, "");
    }

    #[test]
    fn parse_port_accepts_a_port_in_range_and_ignores_surrounding_whitespace() {
        assert_eq!(parse_port("1"), Some(1));
        assert_eq!(parse_port("  8563 "), Some(8563));
        assert_eq!(parse_port("65535"), Some(65535));
    }

    #[test]
    fn parse_port_rejects_zero_out_of_range_and_non_numeric_input() {
        assert_eq!(parse_port("0"), None);
        assert_eq!(parse_port("65536"), None);
        assert_eq!(parse_port("-1"), None);
        assert_eq!(parse_port("abc"), None);
        assert_eq!(parse_port(""), None);
    }

    #[test]
    fn blank_to_none_maps_an_empty_string_to_none_and_keeps_every_other_value() {
        assert_eq!(blank_to_none(String::new()), None);
        assert_eq!(blank_to_none(" ".to_string()), Some(" ".to_string()));
        assert_eq!(blank_to_none("x".to_string()), Some("x".to_string()));
    }

    fn config_of(names: &[&str]) -> config::Config {
        names
            .iter()
            .map(|name| {
                let mut profile = minimal_profile();
                profile.default = Some(true);
                ((*name).to_string(), profile)
            })
            .collect()
    }

    fn defaults_of(config: &config::Config) -> Vec<(&str, Option<bool>)> {
        config
            .iter()
            .map(|(name, profile)| (name.as_str(), profile.default))
            .collect()
    }

    #[test]
    fn clear_other_defaults_drops_the_flag_from_every_profile_when_none_is_kept() {
        let mut config = config_of(&["a", "b", "c"]);

        clear_other_defaults(&mut config, None);

        assert_eq!(
            defaults_of(&config),
            vec![("a", None), ("b", None), ("c", None)]
        );
    }

    #[test]
    fn clear_other_defaults_keeps_the_flag_on_the_named_profile_only() {
        let mut config = config_of(&["a", "b", "c"]);

        clear_other_defaults(&mut config, Some("b"));

        assert_eq!(
            defaults_of(&config),
            vec![("a", None), ("b", Some(true)), ("c", None)]
        );
    }

    #[test]
    fn inquire_port_returns_the_first_valid_answer_and_offers_the_given_default() {
        let mut prompter = ScriptedPrompter::new().texts(&["1234"]);

        let port = inquire_port(&mut prompter, "8563").unwrap();

        assert_eq!(port, 1234);
        assert_eq!(prompter.asked, vec![asked_text("Port:", Some("8563"))]);
    }

    #[test]
    fn inquire_port_re_prompts_with_a_notice_until_the_answer_is_a_valid_port() {
        let mut prompter = ScriptedPrompter::new().texts(&["0", "abc", "70000", "1234"]);

        let port = inquire_port(&mut prompter, "8563").unwrap();

        assert_eq!(port, 1234);
        assert_eq!(
            prompter.asked,
            vec![
                asked_text("Port:", Some("8563")),
                asked_notice("  not a valid port — enter 1..65535"),
                asked_text("Port:", Some("8563")),
                asked_notice("  not a valid port — enter 1..65535"),
                asked_text("Port:", Some("8563")),
                asked_notice("  not a valid port — enter 1..65535"),
                asked_text("Port:", Some("8563")),
            ]
        );
    }

    #[test]
    fn prompt_profile_name_offers_default_as_the_default_when_no_profile_exists() {
        let mut prompter = ScriptedPrompter::new().texts(&["default"]);

        let name = prompt_profile_name(&mut prompter, &config::Config::new()).unwrap();

        assert_eq!(name, "default");
        assert_eq!(
            prompter.asked,
            vec![asked_text("Profile name:", Some("default"))]
        );
    }

    #[test]
    fn prompt_profile_name_offers_no_default_when_profiles_already_exist() {
        let mut prompter = ScriptedPrompter::new().texts(&["fresh"]);

        let name = prompt_profile_name(&mut prompter, &config_of(&["taken"])).unwrap();

        assert_eq!(name, "fresh");
        assert_eq!(prompter.asked, vec![asked_text("Profile name:", None)]);
    }

    #[test]
    fn prompt_profile_name_re_prompts_with_a_notice_when_the_name_is_already_taken() {
        let mut prompter = ScriptedPrompter::new().texts(&["taken", "fresh"]);

        let name = prompt_profile_name(&mut prompter, &config_of(&["taken"])).unwrap();

        assert_eq!(name, "fresh");
        assert_eq!(
            prompter.asked,
            vec![
                asked_text("Profile name:", None),
                asked_notice("  'taken' already exists — choose another name"),
                asked_text("Profile name:", None),
            ]
        );
    }

    #[test]
    fn prompt_profile_name_re_prompts_with_the_validation_error_when_the_name_is_invalid() {
        let mut prompter = ScriptedPrompter::new().texts(&["_bad", "good"]);
        let validation_error = config::validate_profile_name("_bad")
            .unwrap_err()
            .to_string();

        let name = prompt_profile_name(&mut prompter, &config::Config::new()).unwrap();

        assert_eq!(name, "good");
        assert_eq!(
            prompter.asked,
            vec![
                asked_text("Profile name:", Some("default")),
                asked_notice(&format!("  {}", validation_error)),
                asked_text("Profile name:", Some("default")),
            ]
        );
    }

    #[test]
    fn prompt_new_password_returns_the_password_when_both_entries_match() {
        let mut prompter = ScriptedPrompter::new().passwords(&["secret", "secret"]);

        let password = prompt_new_password(&mut prompter).unwrap();

        assert_eq!(password, "secret");
        assert_eq!(
            prompter.asked,
            vec![
                asked_password("Password: "),
                asked_password("Confirm password: "),
            ]
        );
    }

    #[test]
    fn prompt_new_password_re_prompts_with_a_notice_when_the_password_is_empty() {
        let mut prompter = ScriptedPrompter::new().passwords(&["", "secret", "secret"]);

        let password = prompt_new_password(&mut prompter).unwrap();

        assert_eq!(password, "secret");
        assert_eq!(
            prompter.asked,
            vec![
                asked_password("Password: "),
                asked_notice("  password cannot be empty"),
                asked_password("Password: "),
                asked_password("Confirm password: "),
            ]
        );
    }

    #[test]
    fn prompt_new_password_re_prompts_with_a_notice_when_the_two_entries_differ() {
        let mut prompter = ScriptedPrompter::new().passwords(&["one", "two", "same", "same"]);

        let password = prompt_new_password(&mut prompter).unwrap();

        assert_eq!(password, "same");
        assert_eq!(
            prompter.asked,
            vec![
                asked_password("Password: "),
                asked_password("Confirm password: "),
                asked_notice("  passwords did not match — try again"),
                asked_password("Password: "),
                asked_password("Confirm password: "),
            ]
        );
    }

    #[test]
    fn prompt_bucketfs_returns_no_settings_and_asks_nothing_further_when_declined() {
        let mut prompter = ScriptedPrompter::new().confirms(&[false]);

        let settings = prompt_bucketfs(&mut prompter).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: None,
                port: None,
                bucket: None,
                write_password: None,
                read_password: None,
                tls: None,
                validate_certificate: None,
            }
        );
        assert_eq!(prompter.asked, prompt_bucketfs_prompts("2581")[..1]);
    }

    #[test]
    fn prompt_bucketfs_collects_every_setting_when_accepted() {
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .texts(&["bfs.example.com", "3000", "mybucket"])
            .passwords(&["writepw", "readpw"]);

        let settings = prompt_bucketfs(&mut prompter).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: Some("bfs.example.com".to_string()),
                port: Some(3000),
                bucket: Some("mybucket".to_string()),
                write_password: Some("writepw".to_string()),
                read_password: Some("readpw".to_string()),
                tls: None,
                validate_certificate: None,
            }
        );
        let mut expected = prompt_bucketfs_prompts("2581");
        expected.push(asked_password("BucketFS write password (blank to skip): "));
        expected.push(asked_password(
            "BucketFS read password (blank = same as write password): ",
        ));
        assert_eq!(prompter.asked, expected);
    }

    #[test]
    fn prompt_bucketfs_maps_a_blank_host_and_blank_passwords_to_no_value() {
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .texts(&["", "2581", ""])
            .passwords(&["", ""]);

        let settings = prompt_bucketfs(&mut prompter).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: None,
                port: Some(2581),
                bucket: Some(String::new()),
                write_password: None,
                read_password: None,
                tls: None,
                validate_certificate: None,
            }
        );
    }

    #[test]
    fn edit_bucketfs_keeps_every_current_setting_when_the_edit_is_declined() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new().confirms(&[false]);

        let settings = edit_bucketfs(&mut prompter, &current).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: Some("bfs.example.com".to_string()),
                port: Some(2581),
                bucket: Some("mybucket".to_string()),
                write_password: Some("writepw".to_string()),
                read_password: Some("readpw".to_string()),
                tls: Some(true),
                validate_certificate: Some(true),
            }
        );
        assert_eq!(
            prompter.asked,
            edit_bucketfs_prompts("bfs.example.com", "2581", "mybucket")[..1]
        );
    }

    #[test]
    fn edit_bucketfs_offers_the_current_values_as_defaults_and_keeps_both_passwords() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true, false, false])
            .texts(&["newhost", "3000", "newbucket"]);

        let settings = edit_bucketfs(&mut prompter, &current).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: Some("newhost".to_string()),
                port: Some(3000),
                bucket: Some("newbucket".to_string()),
                write_password: Some("writepw".to_string()),
                read_password: Some("readpw".to_string()),
                tls: Some(true),
                validate_certificate: Some(true),
            }
        );
        let mut expected = edit_bucketfs_prompts("bfs.example.com", "2581", "mybucket");
        expected.push(asked_confirm(
            "Change BucketFS write password? (No keeps existing)",
            false,
        ));
        expected.push(asked_confirm(
            "Change BucketFS read password? (No keeps existing)",
            false,
        ));
        assert_eq!(prompter.asked, expected);
    }

    #[test]
    fn edit_bucketfs_clears_a_password_when_the_replacement_is_blank() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true, true, true])
            .texts(&["", "2581", "mybucket"])
            .passwords(&["", ""]);

        let settings = edit_bucketfs(&mut prompter, &current).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: None,
                port: Some(2581),
                bucket: Some("mybucket".to_string()),
                write_password: None,
                read_password: None,
                tls: Some(true),
                validate_certificate: Some(true),
            }
        );
        let mut expected = edit_bucketfs_prompts("bfs.example.com", "2581", "mybucket");
        expected.push(asked_confirm(
            "Change BucketFS write password? (No keeps existing)",
            false,
        ));
        expected.push(asked_password("BucketFS write password (blank = clear): "));
        expected.push(asked_confirm(
            "Change BucketFS read password? (No keeps existing)",
            false,
        ));
        expected.push(asked_password(
            "BucketFS read password (blank = fall back to write password): ",
        ));
        assert_eq!(prompter.asked, expected);
    }

    #[test]
    fn edit_bucketfs_falls_back_to_the_standard_defaults_when_nothing_is_configured_yet() {
        let current = minimal_profile();
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true, false, false])
            .texts(&["", "2581", "default"]);

        let settings = edit_bucketfs(&mut prompter, &current).unwrap();

        assert_eq!(
            settings,
            BucketFsSettings {
                host: None,
                port: Some(2581),
                bucket: Some("default".to_string()),
                write_password: None,
                read_password: None,
                tls: None,
                validate_certificate: None,
            }
        );
        assert_eq!(
            prompter.asked[1..4],
            edit_bucketfs_prompts("", "2581", "default")[1..4]
        );
    }

    #[test]
    fn edit_bucketfs_fails_hard_on_an_invalid_port_without_re_prompting() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .texts(&["bfs.example.com", "nope"]);

        let err = edit_bucketfs(&mut prompter, &current).unwrap_err();

        assert_eq!(err.to_string(), "invalid BucketFS port: nope");
        assert_eq!(
            prompter.asked,
            edit_bucketfs_prompts("bfs.example.com", "2581", "mybucket")[..3]
        );
    }

    #[test]
    fn edit_bucketfs_propagates_an_error_from_the_host_prompt() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .confirms(&[true])
            .text_error("cancelled");

        let err = edit_bucketfs(&mut prompter, &current).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }

    fn blank_init_args() -> InitArgs {
        InitArgs {
            name: None,
            host: None,
            port: None,
            user: None,
            schema: None,
            certificate_fingerprint: None,
            default: false,
            no_bucketfs: false,
        }
    }

    #[test]
    fn init_profile_prompts_for_every_field_that_was_not_supplied() {
        let mut prompter = ScriptedPrompter::new()
            .texts(&["newprofile", "db.example.com", "1234", "admin", "analytics"])
            .passwords(&["secret", "secret"])
            .confirms(&[false, false, true, false]);

        let (name, profile) =
            init_profile(&mut prompter, blank_init_args(), &config_of(&["other"])).unwrap();

        assert_eq!(name, "newprofile");
        assert_eq!(profile.host, "db.example.com");
        assert_eq!(profile.port, Some(1234));
        assert_eq!(profile.user, "admin");
        assert_eq!(profile.password, "secret");
        assert_eq!(profile.schema, Some("analytics".to_string()));
        assert_eq!(profile.tls, Some(false));
        assert_eq!(profile.validate_certificate, Some(false));
        assert_eq!(profile.certificate_fingerprint, None);
        assert_eq!(profile.default, Some(true));
        assert_eq!(profile.bfs_host, None);
        assert_eq!(profile.bfs_port, None);
        let mut expected = vec![
            asked_text("Profile name:", None),
            asked_text("Host:", None),
            asked_text("Port:", Some("8563")),
            asked_text("User:", None),
            asked_password("Password: "),
            asked_password("Confirm password: "),
            asked_text("Schema (optional):", Some("")),
            asked_confirm("Enable TLS?", true),
            asked_confirm("Validate server certificate?", true),
            asked_confirm("Set as the default profile?", false),
        ];
        expected.extend_from_slice(&prompt_bucketfs_prompts("2581")[..1]);
        assert_eq!(prompter.asked, expected);
    }

    #[test]
    fn init_profile_skips_the_prompt_for_every_field_supplied_on_the_command_line() {
        let args = InitArgs {
            name: Some("prod".to_string()),
            host: Some("prod.example.com".to_string()),
            port: Some(9999),
            user: Some("operator".to_string()),
            schema: Some("staging".to_string()),
            certificate_fingerprint: Some("ab:cd".to_string()),
            default: true,
            no_bucketfs: true,
        };
        let mut prompter = ScriptedPrompter::new()
            .passwords(&["secret", "secret"])
            .confirms(&[true, true]);

        let (name, profile) = init_profile(&mut prompter, args, &config_of(&["other"])).unwrap();

        assert_eq!(name, "prod");
        assert_eq!(profile.host, "prod.example.com");
        assert_eq!(profile.port, Some(9999));
        assert_eq!(profile.user, "operator");
        assert_eq!(profile.schema, Some("staging".to_string()));
        assert_eq!(profile.certificate_fingerprint, Some("ab:cd".to_string()));
        assert_eq!(profile.default, Some(true));
        assert_eq!(profile.bfs_bucket, None);
        assert_eq!(
            prompter.asked,
            vec![
                asked_password("Password: "),
                asked_password("Confirm password: "),
                asked_confirm("Enable TLS?", true),
                asked_confirm("Validate server certificate?", true),
            ]
        );
    }

    #[test]
    fn init_profile_treats_an_empty_schema_argument_as_no_schema_and_asks_nothing() {
        let args = InitArgs {
            name: Some("prod".to_string()),
            host: Some("h".to_string()),
            port: Some(1),
            user: Some("u".to_string()),
            schema: Some(String::new()),
            default: true,
            no_bucketfs: true,
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new()
            .passwords(&["p", "p"])
            .confirms(&[true, true]);

        let (_, profile) = init_profile(&mut prompter, args, &config_of(&["other"])).unwrap();

        assert_eq!(profile.schema, None);
        assert!(!prompter
            .asked
            .contains(&asked_text("Schema (optional):", Some(""))));
    }

    #[test]
    fn init_profile_maps_a_blank_schema_answer_to_no_schema() {
        let args = InitArgs {
            name: Some("prod".to_string()),
            host: Some("h".to_string()),
            port: Some(1),
            user: Some("u".to_string()),
            default: true,
            no_bucketfs: true,
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new()
            .texts(&[""])
            .passwords(&["p", "p"])
            .confirms(&[true, true]);

        let (_, profile) = init_profile(&mut prompter, args, &config_of(&["other"])).unwrap();

        assert_eq!(profile.schema, None);
        assert_eq!(
            prompter.asked[2],
            asked_text("Schema (optional):", Some(""))
        );
    }

    #[test]
    fn init_profile_marks_the_first_profile_default_without_asking() {
        let mut prompter = ScriptedPrompter::new()
            .texts(&["default", "h", "8563", "u", ""])
            .passwords(&["p", "p"])
            .confirms(&[true, true, false]);

        let (_, profile) =
            init_profile(&mut prompter, blank_init_args(), &config::Config::new()).unwrap();

        assert_eq!(profile.default, Some(true));
        assert!(!prompter
            .asked
            .contains(&asked_confirm("Set as the default profile?", false)));
        assert_eq!(
            prompter.asked[0],
            asked_text("Profile name:", Some("default"))
        );
    }

    #[test]
    fn init_profile_leaves_the_default_flag_unset_when_the_prompt_is_declined() {
        let mut prompter = ScriptedPrompter::new()
            .texts(&["fresh", "h", "8563", "u", ""])
            .passwords(&["p", "p"])
            .confirms(&[true, true, false, false]);

        let (_, profile) =
            init_profile(&mut prompter, blank_init_args(), &config_of(&["other"])).unwrap();

        assert_eq!(profile.default, None);
        assert!(prompter
            .asked
            .contains(&asked_confirm("Set as the default profile?", false)));
    }

    #[test]
    fn init_profile_stores_the_bucketfs_settings_collected_when_bucketfs_is_accepted() {
        let args = InitArgs {
            name: Some("prod".to_string()),
            host: Some("h".to_string()),
            port: Some(1),
            user: Some("u".to_string()),
            schema: Some("s".to_string()),
            default: true,
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new()
            .texts(&["bfs.host", "3000", "bucket"])
            .passwords(&["p", "p", "wpw", ""])
            .confirms(&[true, true, true]);

        let (_, profile) = init_profile(&mut prompter, args, &config_of(&["other"])).unwrap();

        assert_eq!(profile.bfs_host, Some("bfs.host".to_string()));
        assert_eq!(profile.bfs_port, Some(3000));
        assert_eq!(profile.bfs_bucket, Some("bucket".to_string()));
        assert_eq!(profile.bfs_write_password, Some("wpw".to_string()));
        assert_eq!(profile.bfs_read_password, None);
        assert_eq!(profile.bfs_tls, None);
        assert_eq!(profile.bfs_validate_certificate, None);
    }

    #[test]
    fn init_profile_rejects_a_name_that_already_exists() {
        let args = InitArgs {
            name: Some("taken".to_string()),
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new();

        let err = init_profile(&mut prompter, args, &config_of(&["taken"])).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Profile 'taken' already exists. Remove it first with `exapump profile remove taken`"
        );
        assert_eq!(prompter.asked, vec![]);
    }

    #[test]
    fn init_profile_rejects_an_invalid_name_argument_before_prompting() {
        let args = InitArgs {
            name: Some("_bad".to_string()),
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new();

        let err = init_profile(&mut prompter, args, &config::Config::new()).unwrap_err();

        assert_eq!(
            err.to_string(),
            config::validate_profile_name("_bad")
                .unwrap_err()
                .to_string()
        );
        assert_eq!(prompter.asked, vec![]);
    }

    #[test]
    fn init_profile_re_prompts_for_the_port_until_it_is_valid() {
        let args = InitArgs {
            name: Some("prod".to_string()),
            host: Some("h".to_string()),
            user: Some("u".to_string()),
            schema: Some("s".to_string()),
            default: true,
            no_bucketfs: true,
            ..blank_init_args()
        };
        let mut prompter = ScriptedPrompter::new()
            .texts(&["abc", "1234"])
            .passwords(&["p", "p"])
            .confirms(&[true, true]);

        let (_, profile) = init_profile(&mut prompter, args, &config_of(&["other"])).unwrap();

        assert_eq!(profile.port, Some(1234));
        assert_eq!(
            prompter.asked[..3],
            [
                asked_text("Port:", Some("8563")),
                asked_notice("  not a valid port — enter 1..65535"),
                asked_text("Port:", Some("8563")),
            ]
        );
    }

    #[test]
    fn edit_profile_offers_every_current_value_as_the_prompt_default() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&[
                "db.example.com",
                "1234",
                "admin",
                "analytics",
                "ab:cd:ef",
                "bfs.example.com",
                "2581",
                "mybucket",
            ])
            .confirms(&[false, false, false, true, true, false, false]);

        let updated = edit_profile(&mut prompter, &current, false).unwrap();

        assert_eq!(updated.password, "secret");
        assert_eq!(updated.default, Some(true));
        assert_eq!(updated.bfs_write_password, Some("writepw".to_string()));
        assert_eq!(updated.bfs_read_password, Some("readpw".to_string()));
        assert_eq!(updated.bfs_tls, Some(true));
        assert_eq!(updated.bfs_validate_certificate, Some(true));
        let mut expected = vec![
            asked_text("Host:", Some("db.example.com")),
            asked_text("Port:", Some("1234")),
            asked_text("User:", Some("admin")),
            asked_confirm("Change password? (No keeps the existing password)", false),
            asked_text("Schema (blank = none):", Some("analytics")),
            asked_confirm("Enable TLS?", false),
            asked_confirm("Validate server certificate?", false),
            asked_text("Certificate fingerprint (blank = none):", Some("ab:cd:ef")),
            asked_confirm("Set as the default profile?", true),
        ];
        expected.extend(edit_bucketfs_prompts("bfs.example.com", "2581", "mybucket"));
        expected.push(asked_confirm(
            "Change BucketFS write password? (No keeps existing)",
            false,
        ));
        expected.push(asked_confirm(
            "Change BucketFS read password? (No keeps existing)",
            false,
        ));
        assert_eq!(prompter.asked, expected);
    }

    #[test]
    fn edit_profile_replaces_the_password_when_the_change_is_accepted() {
        let current = minimal_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["h", "8563", "u", "", ""])
            .passwords(&["new", "new"])
            .confirms(&[true, true, true, false]);

        let updated = edit_profile(&mut prompter, &current, true).unwrap();

        assert_eq!(updated.password, "new");
        assert_eq!(
            prompter.asked[3..6],
            [
                asked_confirm("Change password? (No keeps the existing password)", false),
                asked_password("Password: "),
                asked_password("Confirm password: "),
            ]
        );
    }

    #[test]
    fn edit_profile_clears_the_schema_and_the_fingerprint_when_the_answers_are_blank() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["h", "1234", "u", "", ""])
            .confirms(&[false, true, true, false]);

        let updated = edit_profile(&mut prompter, &current, true).unwrap();

        assert_eq!(updated.schema, None);
        assert_eq!(updated.certificate_fingerprint, None);
        assert_eq!(updated.default, None);
    }

    #[test]
    fn edit_profile_keeps_the_current_bucketfs_settings_when_bucketfs_is_skipped() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["h", "1234", "u", "s", "fp"])
            .confirms(&[false, false, false, false]);

        let updated = edit_profile(&mut prompter, &current, true).unwrap();

        assert_eq!(updated.bfs_host, Some("bfs.example.com".to_string()));
        assert_eq!(updated.bfs_port, Some(2581));
        assert_eq!(updated.bfs_bucket, Some("mybucket".to_string()));
        assert_eq!(updated.bfs_write_password, Some("writepw".to_string()));
        assert_eq!(updated.bfs_read_password, Some("readpw".to_string()));
        assert_eq!(updated.bfs_tls, Some(true));
        assert_eq!(updated.bfs_validate_certificate, Some(true));
        assert_eq!(
            prompter.asked.last(),
            Some(&asked_confirm("Set as the default profile?", true))
        );
    }

    #[test]
    fn edit_profile_re_prompts_for_the_port_until_it_is_valid() {
        let current = minimal_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["h", "0", "8563", "u", "", ""])
            .confirms(&[false, true, true, false]);

        let updated = edit_profile(&mut prompter, &current, true).unwrap();

        assert_eq!(updated.port, Some(8563));
        assert_eq!(
            prompter.asked[..4],
            [
                asked_text("Host:", Some("localhost")),
                asked_text("Port:", Some("8563")),
                asked_notice("  not a valid port — enter 1..65535"),
                asked_text("Port:", Some("8563")),
            ]
        );
    }

    #[test]
    fn edit_profile_rejects_a_blank_answer_for_a_required_field() {
        let current = minimal_profile();
        let mut prompter = ScriptedPrompter::new().texts(&[""]);

        let err = edit_profile(&mut prompter, &current, true).unwrap_err();

        assert_eq!(err.to_string(), "Host is required");
    }

    #[test]
    fn edit_profile_propagates_an_error_from_the_validate_certificate_confirmation() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["newhost", "1234", "newuser", ""])
            .confirms(&[false, true])
            .confirm_error("cancelled");

        let err = edit_profile(&mut prompter, &current, true).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn edit_profile_propagates_an_error_from_the_certificate_fingerprint_prompt() {
        let current = full_profile();
        let mut prompter = ScriptedPrompter::new()
            .texts(&["newhost", "1234", "newuser", ""])
            .confirms(&[false, true, true])
            .text_error("cancelled");

        let err = edit_profile(&mut prompter, &current, true).unwrap_err();

        assert_eq!(err.to_string(), "cancelled");
    }
}
