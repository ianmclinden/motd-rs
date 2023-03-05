use clap::{crate_version, Arg, ArgAction};
use colored::Colorize;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{exit, Command},
};

const PREFIX: &str = env!("PREFIX");

fn main() {
    let prefix = if PREFIX.is_empty() { "/" } else { PREFIX };
    let stamp_dir = Path::new(prefix)
        .join("var")
        .join("lib")
        .join("update-notifier");
    let stamp_file = stamp_dir.join("updates-available");

    let matches = clap::Command::new("update-notifier")
        .about("track available brew updates for MOTD")
        .long_about(format!(
            "
Track available brew updates for message of the day.

When called, stampfile '{}' is updated with the current state
of packages. This stampfile can then be read by an MOTD fragment.",
            stamp_file.to_string_lossy(),
        ))
        .arg(
            Arg::new("stamp")
                .long("stamp")
                .short('s')
                .help("Print the update stampfile path and exit")
                .action(ArgAction::SetTrue),
        )
        .version(crate_version!())
        .get_matches();

    // Path only
    if matches.get_flag("stamp") {
        println!("{}", stamp_file.to_string_lossy());
        exit(0);
    }
    // First, update
    if Command::new("brew").arg("update").output().is_ok() {
        // Then, get outdated pacakge list
        if let Ok(output) = Command::new("brew").arg("outdated").output() {
            // Create the update directory
            fs::create_dir_all(stamp_dir).expect("Could not create stamp directory.");
            let mut stamp = fs::File::create(stamp_file).expect("Could not create stampfile");

            let outdated = String::from_utf8_lossy(&output.stdout).lines().count();
            if outdated > 0 {
                writeln!(
                    &mut stamp,
                    "You have {} outdated formula{} installed.",
                    outdated.to_string().bold(),
                    if outdated > 1 { "s" } else { "" }
                )
                .unwrap();
                writeln!(
                    &mut stamp,
                    "You can upgrade {} with {}\nor list {} with {}",
                    if outdated > 1 { "them" } else { "it" },
                    "brew upgrade".bold(),
                    if outdated > 1 { "them" } else { "it" },
                    "brew outdated".bold()
                )
                .unwrap();
            }
        }
    }
}
