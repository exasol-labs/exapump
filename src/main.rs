mod cli;
mod commands;
mod connection;
mod format;
mod size;
mod split;

use std::io::Write;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Upload(args)) => {
            commands::upload::run(args).await?;
        }
        Some(Commands::Sql(args)) => {
            commands::sql::run(args).await?;
        }
        Some(Commands::Export(args)) => {
            commands::export::run(args).await?;
        }
        Some(Commands::Interactive(args)) => {
            commands::interactive::run(args).await?;
        }
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            cmd.print_help()?;
            writeln!(std::io::stdout())?;
            std::process::exit(2);
        }
    }

    Ok(())
}
