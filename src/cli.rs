use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lazarus", version, about = "CLI training app")]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Session-scoped commands
    #[command(subcommand, alias = "s")]
    Session(SessionCmd),
}


#[derive(Subcommand)]
pub enum SessionCmd {
    /// Start a session
    #[command(alias = "ss")]
    Start(StartArgs)
}

#[derive(Args)]
pub struct StartArgs {
    pub program: String,
    pub block:   String,
    pub week:    Option<i32>,
}
