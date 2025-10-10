use clap::Parser;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, exit},
    sync::LazyLock,
};

static STAMP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let prefix = match option_env!("PREFIX") {
        Some(v) if !v.is_empty() => v,
        _ => "/",
    };
    Path::new(prefix)
        .join("var")
        .join("lib")
        .join("update-notifier")
});

static STAMP_FILE: LazyLock<PathBuf> = LazyLock::new(|| STAMP_DIR.join("updates-available"));

static HELP_LONG: LazyLock<String> = LazyLock::new(|| {
    format!(
        "Track available brew updates for message of the day.

When called, stampfile '{}' is updated with the current state
of packages. This stampfile can then be read by an MOTD fragment.",
        STAMP_FILE.display()
    )
});

/// Track available brew updates for MOTD
#[derive(Parser, Debug)]
#[command(version, about, long_about = HELP_LONG.as_str())]
struct Args {
    /// Print the update stampfile path and exit
    #[arg(short, long, default_value_t = false)]
    path: bool,
}

fn main() {
    let args = Args::parse();

    // Path only
    if args.path {
        println!("{}", STAMP_FILE.display());
        exit(0);
    }

    // First, update
    if Command::new("brew").arg("update").output().is_ok() {
        // Then, get outdated package list
        if let Ok(output) = Command::new("brew").arg("outdated").output() {
            // Create the update directory
            fs::create_dir_all(STAMP_DIR.as_path()).expect("Could not create stamp directory.");
            let mut stamp =
                fs::File::create(STAMP_FILE.as_path()).expect("Could not create stampfile");

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
