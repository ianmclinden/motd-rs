use clap::Parser;
use std::{
    path::{Path, PathBuf},
    process::{exit, Command},
    sync::OnceLock,
};
use walkdir::WalkDir;

fn motd_dir() -> &'static PathBuf {
    static MOTD_DIR: OnceLock<PathBuf> = OnceLock::new();
    MOTD_DIR.get_or_init(|| {
        let prefix = match option_env!("PREFIX") {
            Some(v) if !v.is_empty() => v,
            _ => "/",
        };
        Path::new(prefix).join("etc").join("update-motd.d")
    })
}

fn help_long() -> &'static String {
    static HELP_LONG: OnceLock<String> = OnceLock::new();
    HELP_LONG.get_or_init(|| {
        format!(
            "Dynamic message of the day generation.

Any executable scripts in '{}/*' are 
executed as the current user, and concatenated to the MOTD. These scripts must
be executable, and must emit information on standard out.",
            motd_dir().to_string_lossy()
        )
    })
}

/// Dynamic MOTD generation
#[derive(Parser, Debug)]
#[command(version, about, long_about = help_long())]
struct Args {
    /// Print the MOTD_DIR path and exit
    #[arg(short, long, default_value_t = false)]
    path: bool,
}

fn main() {
    let args = Args::parse();

    // Path only
    if args.path {
        println!("{}", motd_dir().to_string_lossy());
        exit(0);
    }

    let mut motd_buf: String = String::new();

    // Execute and concat MOTD fragments
    for fragment in WalkDir::new(motd_dir())
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.metadata().is_ok()
                && e.metadata().unwrap().is_file()
                && !e.file_name().to_string_lossy().ends_with(".default")
        })
    {
        // Lazily append only successes
        if let Ok(output) = Command::new(fragment.into_path()).output() {
            motd_buf.push_str(&String::from_utf8_lossy(&output.stdout));
        }
    }

    print!("{motd_buf}");
}
