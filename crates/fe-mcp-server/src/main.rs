use clap::Parser;

mod mcp;
mod server;
mod tools;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // All logging goes to stderr so stdout is reserved for MCP JSON-RPC.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            project_root,
            framework: _,
        } => {
            let project_root = project_root.canonicalize()?;
            tracing::info!("Starting fe-tools MCP server at {:?}", project_root);
            server::run(project_root).await?;
        }
    }

    Ok(())
}
