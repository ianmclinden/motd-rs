use clap::Parser;
use futures::future::join_all;
use std::{
    path::{Path, PathBuf},
    process::exit,
    sync::LazyLock,
};
use tokio::process::Command;
use walkdir::WalkDir;

static MOTD_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let prefix = match option_env!("PREFIX") {
        Some(v) if !v.is_empty() => v,
        _ => "/",
    };
    Path::new(prefix).join("etc").join("update-motd.d")
});

static HELP_LONG: LazyLock<String> = LazyLock::new(|| {
    format!(
        "Dynamic message of the day generation.

Any executable scripts in '{}/*' are
executed as the current user, and concatenated to the MOTD. These scripts must
be executable, and must emit information on standard out.",
        MOTD_DIR.display()
    )
});

/// Dynamic MOTD generation
#[derive(Parser, Debug)]
#[command(version, about, long_about = HELP_LONG.as_str())]
struct Args {
    /// Print the `MOTD_DIR` path and exit
    #[arg(short, long, default_value_t = false)]
    path: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Path only
    if args.path {
        println!("{}", MOTD_DIR.display());
        exit(0);
    }

    // Execute and concat MOTD fragments
    let motd_fragments = WalkDir::new(MOTD_DIR.as_path())
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.metadata().is_ok()
                && e.metadata().unwrap().is_file()
                && !e.file_name().to_string_lossy().ends_with(".default")
        })
        .map(|f| Command::new(f.path()).output());

    // Join output is ordered, keep output from successes
    for output in (join_all(motd_fragments).await).into_iter().flatten() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
