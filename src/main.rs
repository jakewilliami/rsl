use clap::{ArgAction, Parser, crate_authors, crate_name, crate_version};

mod clean;
mod clipboard;
mod resolve;

#[derive(Parser)]
#[command(
    name = crate_name!(),
    author = crate_authors!(", "),
    version = crate_version!(),
)]
/// Resolve share link to canonical form
struct Cli {
    /// URL to resolve
    #[arg(
        action = ArgAction::Set,
        num_args = 1,
        value_name = "URL",
    )]
    url: String,
}

#[tokio::main]
async fn main() {
    // TODO: option to remove scheme and subdomains
    // TODO: option to ignore input validation; just follow redirects and remove query parameters
    let cli = Cli::parse();

    match resolve::resolve(&cli.url)
        .await
        .and_then(|resolved_url| clean::clean_url(&resolved_url).map_err(|e| e.into()))
    {
        Ok(url) => {
            clipboard::copy(&url);
            println!("{}", url);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
