use clap::{ArgAction, Parser};
use futures::future::join_all;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::{
    error::Error,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::LazyLock,
};

#[cfg(feature = "homebrew")]
mod brew {
    use serde::Deserialize;
    use serde_json::Value;
    use tokio::process::Command;

    #[derive(Deserialize)]
    struct OutdatedEntries {
        #[serde(default)]
        formulae: Vec<Value>,
        #[serde(default)]
        casks: Vec<Value>,
    }

    /// Check for updates from homebrew
    pub async fn generate_stamp() -> Result<Option<String>, String> {
        log::debug!("Checking for outdated homebrew packages");

        log::trace!("Updating homebrew (`brew update-if-needed`)");
        let updated = Command::new("brew").arg("update-if-needed").output().await;
        match updated {
            Ok(s) if s.status.success() => {}
            _ => return Err("Could not run `brew update`".to_string()),
        }

        log::trace!("Getting homebrew outdated package list (`brew outdated`)");
        let output = Command::new("brew")
            .arg("outdated")
            .arg("--json=v2")
            .output()
            .await
            .map_err(|_| "Could not run `brew outdated`")?
            .stdout;

        let outdated: OutdatedEntries = serde_json::from_slice(&output)
            .map_err(|_| "Output from `brew outdated` was not valid JSON")?;

        let outdated_formulae = outdated.formulae.len();
        log::debug!("Found {outdated_formulae} outdated homebrew formulae");
        let outdated_casks = outdated.casks.len();
        log::debug!("Found {outdated_casks} outdated homebrew casks");

        let outdated = outdated_formulae + outdated_casks;
        if outdated > 0 {
            let formulas = if outdated > 1 { "formulas" } else { "formula" };
            let them = if outdated > 1 { "them" } else { "it" };

            Ok(Some(format!(
                "You have {outdated} outdated {formulas} installed.\n\nYou can upgrade {them} with `brew upgrade`\nor list {them} with `brew outdated`\n"
            )))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature = "cargo")]
mod cargo {
    use std::{env, path::PathBuf};

    use cargo_update::ops as cu;
    use futures::future::join_all;

    /// Check for updates from installed cargo packages
    ///
    /// Minimal implementation of <https://github.com/nabijaczleweli/cargo-update/blob/master/src/main.rs>
    pub async fn generate_stamp() -> Result<Option<String>, String> {
        log::debug!("Checking for outdated cargo packages");

        let cargo_dir = env::var("HOME")
            .map(|c| PathBuf::from(format!("{c}/.cargo")))
            .map_err(|_| "Could not find cargo home directory")?;

        let crates_file = cu::crates_file_in(&cargo_dir);
        let http_proxy = cu::find_proxy(&crates_file);
        let configuration = cu::PackageConfig::read(
            &crates_file.with_file_name(".install_config.toml"),
            &crates_file.with_file_name(".crates2.json"),
        )
        .map_err(|(e, _r)| format!("Failed to read config: {e}"))?;
        let cargo_config = cu::CargoConfig::load(&crates_file);

        log::trace!(
            "Loaded crates & configuration from '{}'",
            cargo_dir.display()
        );

        let updaters = cu::installed_registry_packages(&crates_file)
            .into_iter()
            .flat_map(|rp| {
                let sparse = cargo_config.registries_crates_io_protocol_sparse;
                cu::get_index_url(&crates_file, &rp.registry, sparse).map(|iu| (rp, iu))
            })
            .flat_map(|(rp, iu)| {
                let (repo_url, sparse, _) = &iu;
                cu::assert_index_path(&cargo_dir, repo_url, *sparse).map(|path| (rp, iu, path))
            })
            .flat_map(|(rp, iu, path)| {
                let (_, sparse, _) = iu;
                cu::open_index_repository(&path, sparse).map(|r| (rp, iu, r))
            })
            .flat_map(|(rp, iu, mut r)| {
                let (repo_url, sparse, repo_name) = &iu;
                let auth_providers = cu::auth_providers(
                    &crates_file,
                    None,
                    &cargo_config.sparse_registries,
                    *sparse,
                    repo_name,
                    repo_url,
                );
                // TODO: May be more efficient to keep Map<url, [rp]> and update once per?
                cu::update_index(
                    &mut r,
                    repo_url,
                    [&rp.name].iter(),
                    http_proxy.as_deref(),
                    cargo_config.net_git_fetch_with_cli,
                    &cargo_config.http,
                    auth_providers.r#try().as_deref(),
                    &mut Box::new(std::io::sink()),
                )
                .map(|()| (rp, r))
            })
            .map(|(mut rp, r)| {
                let configuration = configuration.clone();
                tokio::spawn(async move {
                    if let Some(cfg) = configuration.get(&rp.name)
                        && let Ok(rt) = cu::parse_registry_head(&r)
                    {
                        log::trace!("Pulling updates for crate {}", rp.name);
                        rp.pull_version(&rt, &r, cfg.install_prereleases);

                        rp.needs_update(cfg.target_version.as_ref(), cfg.install_prereleases, false)
                            .then_some(())
                    } else {
                        None
                    }
                })
            });

        let outdated = join_all(updaters)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Failed to join cargo update task")?
            .iter()
            .flatten()
            .count();

        log::debug!("Found {outdated} outdated cargo packages");
        if outdated > 0 {
            let packages = if outdated > 1 { "packages" } else { "package" };
            let them = if outdated > 1 { "them" } else { "it" };

            Ok(Some(format!(
                "You have {outdated} outdated cargo {packages} installed.\n\nYou can upgrade {them} with `cargo install-update --all`\nor list {them} with `cargo install-update --list`\n"
            )))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature = "rustup")]
mod rustup {
    use tokio::process::Command;

    /// Check for updated toolchains from rustup
    pub async fn generate_stamp() -> Result<Option<String>, String> {
        log::debug!("Checking for outdated toolchains from rustup");

        log::trace!("Checking rustup (`rustup check`)");
        let output = Command::new("rustup")
            .arg("check")
            .output()
            .await
            .map_err(|_| "Could not run `rustup check`")?
            .stdout;

        let outdated = String::from_utf8(output)
            .map_err(|_| "Output from `rustup check` was not a string")?
            .lines()
            // TODO: Not the most robust way to do this, but rustup's lib is mostly pub(crate)
            .filter(|line| line.contains("Update available"))
            .count();

        log::debug!("Found {outdated} outdated rust toolchains");
        if outdated > 0 {
            let toolchains = if outdated > 1 {
                "toolchains"
            } else {
                "toolchain"
            };
            let them = if outdated > 1 { "them" } else { "it" };

            Ok(Some(format!(
                "You have {outdated} outdated rust {toolchains} installed.\n\nYou can upgrade {them} with `rustup update`\nor list {them} with `rustup check`\n"
            )))
        } else {
            Ok(None)
        }
    }
}

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
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Print the update stampfile path and exit
    #[arg(short, long, default_value_t = false)]
    path: bool,

    /// Output file to update (or 'stdout', 'stderr')
    #[arg(short, long, default_value = STAMP_FILE.as_os_str())]
    output: String,

    /// Don't check homebrew for updates
    #[cfg(feature = "homebrew")]
    #[arg(long, default_value_t = false)]
    no_brew: bool,

    /// Don't check cargo for updates
    #[cfg(feature = "cargo")]
    #[arg(long, default_value_t = false)]
    no_cargo: bool,

    /// Don't check rustup for updates
    #[cfg(feature = "rustup")]
    #[arg(long, default_value_t = false)]
    no_rustup: bool,

    /// Increase verbosity
    #[arg(short, long, action=ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.path {
        println!("{}", STAMP_FILE.display());
        return Ok(());
    }

    let log_level = match args.verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    SimpleLogger::new().with_level(log_level).init()?;
    log::debug!("Log level {log_level}");

    let mut stamps = Vec::new();
    #[cfg(feature = "homebrew")]
    if !args.no_brew {
        stamps.push(tokio::spawn(brew::generate_stamp()));
    }
    #[cfg(feature = "cargo")]
    if !args.no_cargo {
        stamps.push(tokio::spawn(cargo::generate_stamp()));
    }
    #[cfg(feature = "rustup")]
    if !args.no_rustup {
        stamps.push(tokio::spawn(rustup::generate_stamp()));
    }

    if stamps.is_empty() {
        log::debug!("No toolchain-specific updaters enabled");
        return Ok(());
    }

    log::trace!("Dispatching toolchain-specific updaters");
    let stamps = join_all(stamps)
        .await
        .into_iter()
        .map(|handle| handle.expect("failed to join toolchain-specific task"))
        .filter_map(|stamp| stamp.map_err(|e| log::error!("{e}")).ok())
        .flatten()
        .collect::<Vec<_>>();

    if stamps.is_empty() {
        log::debug!("No updates found");
    } else {
        let mut output: Box<dyn Write> = match args.output.to_lowercase().as_str() {
            "stdout" => {
                log::debug!("Writing updates to stdout");
                Box::new(std::io::stdout())
            }
            "stderr" => {
                log::debug!("Writing updates to stderr");
                Box::new(std::io::stderr())
            }
            path => {
                let outpath = PathBuf::from(path);
                if let Some(output_dir) = outpath.parent()
                    && !output_dir.exists()
                {
                    log::trace!("Creating stamps directory '{}'", output_dir.display());
                    fs::create_dir_all(output_dir)
                        .map_err(|_| "Could not create stamp directory.")?;
                }
                let stampfile = File::create(&outpath).map_err(|_| "Could not create stampfile")?;
                log::debug!("Writing updates to stamp file '{}'", outpath.display());
                Box::new(stampfile)
            }
        };

        write!(&mut output, "{}", stamps.join("\n")).map_err(|_| "Could not write updates")?;
    }

    Ok(())
}
