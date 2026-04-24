use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "qinAegis")]
#[command(version = "0.1.0")]
enum Cmd {
    Init,
    Config,
    Explore,
    Generate,
    Run,
    Report,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = Cmd::parse();
    match cmd {
        Cmd::Init => println!("init"),
        Cmd::Config => println!("config"),
        Cmd::Explore => println!("explore"),
        Cmd::Generate => println!("generate"),
        Cmd::Run => println!("run"),
        Cmd::Report => println!("report"),
    }
    Ok(())
}