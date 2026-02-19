use clap::Parser;

mod server;

#[derive(Parser)]
#[command(name = "fe-tools", about = "Frontend MCP tools for AI coding agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the MCP server (stdio transport)
    Serve {
        /// Path to the project root
        #[arg(long, default_value = ".")]
        project_root: std::path::PathBuf,

        /// Frontend framework (react, vue, svelte, auto)
        #[arg(long, default_value = "auto")]
        framework: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("fe_tools=info")
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            project_root,
            framework: _,
        } => {
            let _project_root = project_root.canonicalize()?;
            tracing::info!("Starting fe-tools MCP server");
            // MCP server integration will be added in Phase 3
            eprintln!("MCP server not yet implemented. Use fe-batch library directly.");
        }
    }

    Ok(())
}
