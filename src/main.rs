use clap::{crate_version, Arg, ArgAction};
use std::{
    path::Path,
    process::{exit, Command},
};
use walkdir::WalkDir;

const PREFIX: &str = env!("PREFIX");

fn main() {
    let prefix = if PREFIX.is_empty() { "/" } else { PREFIX };
    let motd_dir = Path::new(prefix).join("etc").join("update-motd.d");

    let matches = clap::Command::new("motd")
        .about("dynamic MOTD generation")
        .long_about(format!(
            "
Dynamic message of the day generation.

Any executable scripts in '{}/*' are 
executed as the current user, and concatenated to the MOTD. These scripts must
be executable, and must emit information on standard out.",
            motd_dir.to_string_lossy(),
        ))
        .arg(
            Arg::new("path")
                .long("path")
                .short('p')
                .help("Print the MOTD_DIR path and exit")
                .action(ArgAction::SetTrue),
        )
        .version(crate_version!())
        .get_matches();

    // Path only
    if matches.get_flag("path") {
        println!("{}", motd_dir.to_string_lossy());
        exit(0);
    }

    let mut motd_buf: String = "".to_string();

    // Execute and concat MOTD fragments
    for fragment in WalkDir::new(motd_dir)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
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
