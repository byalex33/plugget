use clap::Parser;
use plugget::{cli::Cli, commands};
use std::io::{self, Write};

#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let json = args.iter().any(|a| a == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"schema":1,"ok":false,"error":{"message":error.to_string(),"exit_code":error.exit_code()}})
                );
            } else {
                let _ = error.print();
            }
            std::process::exit(error.exit_code());
        }
    };
    let result = match std::env::current_dir() {
        Ok(root) => commands::run(&cli, &root).await,
        Err(e) => Err(e.into()),
    };
    let (text, code) = match result {
        Ok(report) => (
            if cli.json {
                serde_json::to_string(
                    &serde_json::json!({"schema":1,"ok":report.code == 0,"data":report.data}),
                )
                .unwrap()
            } else if cli.quiet {
                String::new()
            } else {
                report.text
            },
            report.code,
        ),
        Err(error) => {
            if cli.json {
                (serde_json::json!({"schema":1,"ok":false,"error":{"message":format!("{error:#}"),"exit_code":1}}).to_string(), 1)
            } else {
                eprintln!("Error: {error:#}");
                (String::new(), 1)
            }
        }
    };
    if !text.is_empty()
        && let Err(e) = writeln!(io::stdout().lock(), "{text}")
        && e.kind() != io::ErrorKind::BrokenPipe
    {
        eprintln!("Output failed: {e}");
        std::process::exit(1);
    }
    std::process::exit(code);
}
