use clap::Parser;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{exit, Command},
    sync::OnceLock,
};

fn stamp_dir() -> &'static PathBuf {
    static STAMP_DIR: OnceLock<PathBuf> = OnceLock::new();
    STAMP_DIR.get_or_init(|| {
        let prefix = match option_env!("PREFIX") {
            Some(v) if !v.is_empty() => v,
            _ => "/",
        };
        Path::new(prefix)
            .join("var")
            .join("lib")
            .join("update-notifier")
    })
}

fn stamp_file() -> &'static PathBuf {
    static STAMP_FILE: OnceLock<PathBuf> = OnceLock::new();
    STAMP_FILE.get_or_init(|| stamp_dir().join("updates-available"))
}

fn help_long() -> &'static String {
    static HELP_LONG: OnceLock<String> = OnceLock::new();
    HELP_LONG.get_or_init(|| {
        format!(
            "Track available brew updates for message of the day.

When called, stampfile '{}' is updated with the current state
of packages. This stampfile can then be read by an MOTD fragment.",
            stamp_file().to_string_lossy()
        )
    })
}

/// "Track available brew updates for MOTD"
#[derive(Parser, Debug)]
#[command(version, about, long_about = help_long())]
struct Args {
    /// Print the update stampfile path and exit
    #[arg(short, long, default_value_t = false)]
    path: bool,
}

fn main() {
    let args = Args::parse();

    // Path only
    if args.path {
        println!("{}", stamp_file().to_string_lossy());
        exit(0);
    }

    // First, update
    if Command::new("brew").arg("update").output().is_ok() {
        // Then, get outdated package list
        if let Ok(output) = Command::new("brew").arg("outdated").output() {
            // Create the update directory
            fs::create_dir_all(stamp_dir()).expect("Could not create stamp directory.");
            let mut stamp = fs::File::create(stamp_file()).expect("Could not create stampfile");

            let outdated = String::from_utf8_lossy(&output.stdout).lines().count();
            if outdated > 0 {
                let formulas = if outdated > 1 { "formulas" } else { "formula" };
                let them = if outdated > 1 { "them" } else { "it" };
                writeln!(
                    &mut stamp,
                    "You have {outdated} outdated {formulas} installed."
                )
                .unwrap();
                writeln!(
                    &mut stamp,
                    "You can upgrade {them} with `brew upgrade`\nor list {them} with `brew outdated`",
                )
                .unwrap();
            }
        }
    }
}
