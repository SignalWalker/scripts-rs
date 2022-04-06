#![feature(path_try_exists)]
#![feature(async_closure)]
#![feature(io_read_to_string)]

use clap::{Parser, Subcommand};
use console::{Color, Style, Term};

use raur::{Package, Raur, SearchBy};
use scripts_rs::init_fern;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    process::{Command, Stdio},
    str::{FromStr, Utf8Error},
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AURError {
    #[error("unrecognized query field: {0}")]
    UnrecognizedField(String),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("makepkg failed")]
    InstallFailed,
    #[error("unrecognized enum option variant")]
    UnrecognizedVariant,
    #[error("pacman command failed")]
    PacmanFailed,
    #[error(transparent)]
    UTF8(#[from] Utf8Error),
    #[error(transparent)]
    Raur(#[from] raur::Error),
    #[error("package not installed: {0}")]
    NotInstalled(String),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error("failed to pull repository: {0}")]
    PullFailed(String),
}

#[derive(Debug)]
pub enum SortBy {
    Name,
    Submitted,
    Modified,
    Votes,
    Popularity,
}

impl FromStr for SortBy {
    type Err = AURError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "name" | "n" => Ok(Self::Name),
            "submitted" | "s" => Ok(Self::Submitted),
            "modified" | "m" => Ok(Self::Modified),
            "votes" | "v" => Ok(Self::Votes),
            "popularity" | "p" => Ok(Self::Popularity),
            _ => Err(AURError::UnrecognizedVariant),
        }
    }
}

#[derive(Debug)]
pub enum SortDir {
    Ascending,
    Descending,
}

impl FromStr for SortDir {
    type Err = AURError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ascending" | "a" => Ok(Self::Ascending),
            "descending" | "d" => Ok(Self::Descending),
            _ => Err(AURError::UnrecognizedVariant),
        }
    }
}

#[derive(Debug, Parser)]
#[clap(version, about = "Manage AUR packages")]
pub struct Args {
    /// output logging level
    #[clap(short, long, default_value = "Info", possible_values = ["Error", "Warn", "Info", "Debug", "Trace"])]
    pub log_lvl: log::LevelFilter,
    #[clap(short, long, env = "AUR_CACHE_DIR")]
    pub cache_dir: String,
    #[clap(subcommand)]
    pub cmd: Cmd,
}

/// Install packages
#[derive(Debug, Subcommand)]
#[clap()]
pub enum Cmd {
    /// install packages
    Install {
        /// whether to install packages as dependencies
        #[clap(short = 'd', long)]
        as_deps: bool,
        /// whether to pull package source
        #[clap(short = 'y', long)]
        refresh: bool,
        /// whether to upgrade packages
        #[clap(short, long)]
        upgrade: bool,
        /// Install missing dependencies with Pacman
        #[clap(short, long)]
        sync_deps: bool,
        /// Remove build dependencies after a successful install
        #[clap(short, long)]
        rm_deps: bool,
        /// Clean build dir after successful install
        #[clap(short, long)]
        clean: bool,
        /// Skip confirmation
        #[clap(long)]
        no_confirm: bool,
        /// Extra packages to install as dependencies
        #[clap(long)]
        deps: Option<Vec<String>>,
        /// Ignore errors in individual packages
        #[clap(short = 'e', long)]
        ignore_errors: bool,
        /// make packages that were previously installed as dependencies into explicitly installed packages
        #[clap(long)]
        make_explicit: bool,
        #[clap()]
        packages: Vec<String>,
    },
    /// clean package cache
    Clean {
        /// clean only packages that aren't currently installed
        #[clap(short = 'u', long)]
        only_uninstalled: bool,
        /// skip delete confirmation
        #[clap(short = 'y', long)]
        no_confirm: bool,
        /// continue through errors
        #[clap(short, long)]
        ignore_errors: bool,
    },
    /// search for packages
    Query {
        /// field by which to search
        #[clap(short, long, default_value = "name-desc", parse(try_from_str = parse_field), possible_values = ["name", "name-desc", "maintainer", "depends", "makedepends", "optdepends", "checkdepends"])]
        field: SearchBy,
        /// display out of date packages
        #[clap(short = 'o', long)]
        keep_outdated: bool,
        /// display unmaintained packages
        #[clap(short = 'u', long)]
        keep_unmaintained: bool,
        /// skip displaying package urls
        #[clap(long)]
        no_url: bool,
        /// sort packages by this field if given
        #[clap(short = 's', long, default_missing_value = "name")]
        sort_by: Option<SortBy>,
        /// direction in which to order sorting
        #[clap(long, default_value = "ascending")]
        sort_dir: SortDir,
        /// Query only installed packages
        #[clap(short = 'i', long)]
        installed: bool,
        /// text for which to search
        #[clap(short = 'k', long, default_value = "")]
        keywords: String,
    },
}

#[allow(clippy::match_str_case_mismatch)]
pub fn parse_field(s: &str) -> Result<SearchBy, AURError> {
    match s.to_lowercase().as_str() {
        "name" => Ok(SearchBy::Name),
        "name-desc" => Ok(SearchBy::NameDesc),
        "maintainer" => Ok(SearchBy::Maintainer),
        "depends" => Ok(SearchBy::Depends),
        "makedepends" => Ok(SearchBy::MakeDepends),
        "optdepends" => Ok(SearchBy::OptDepends),
        "checkdepends" => Ok(SearchBy::CheckDepends),
        _ => Err(AURError::UnrecognizedField(s.to_owned())),
    }
}

pub trait PacExt {
    fn cached(&self, cache_dir: &str) -> std::io::Result<bool>;
    fn installed(&self) -> Result<String, AURError>;
    fn is_dep(&self) -> Result<bool, AURError>;
}

impl PacExt for String {
    fn cached(&self, cache_dir: &str) -> std::io::Result<bool> {
        fs::try_exists(&format!("{}/{}", cache_dir, self))
    }

    fn installed(&self) -> Result<String, AURError> {
        let mut proc = Command::new("pacman")
            .args(["-Q"])
            .arg(self)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let output = proc.wait_with_output()?;
        match output.status.success() {
            false => Err(AURError::NotInstalled(self.clone())),
            true => {
                let stdout = std::str::from_utf8(&output.stdout)?;
                log::debug!(target: "aur::installed()", "{} :: {}", self, stdout);
                Ok(stdout
                    .split(' ')
                    .nth(1)
                    .ok_or_else(|| AURError::NotInstalled(self.clone()))?
                    .trim()
                    .to_owned())
            }
        }
    }

    fn is_dep(&self) -> Result<bool, AURError> {
        let mut proc = Command::new("pacman")
            .args(["-Qd"])
            .arg(self)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let output = proc.wait_with_output()?;
        let stdout = std::str::from_utf8(&output.stdout)?.trim();
        match (output.status.success(), stdout.starts_with("error")) {
            (false, false) => Ok(false),
            (false, true) => Err(AURError::NotInstalled(self.clone())),
            (true, _) => Ok(true),
        }
    }
}

pub trait PackageExt {
    fn git_clone(&self, cache_dir: &str) -> Result<(), git2::Error>;
    fn git_pull(&self, cache_dir: &str) -> Result<bool, AURError>;
    fn install(
        &self,
        cache_dir: &str,
        sync_deps: bool,
        rm_deps: bool,
        clean: bool,
        as_deps: bool,
        confirm: bool,
    ) -> Result<(), AURError>;
}

impl PacExt for Package {
    fn cached(&self, cache_dir: &str) -> std::io::Result<bool> {
        self.name.cached(cache_dir)
    }

    fn installed(&self) -> Result<String, AURError> {
        self.name.installed()
    }

    fn is_dep(&self) -> Result<bool, AURError> {
        self.name.is_dep()
    }
}

impl PackageExt for Package {
    fn git_clone(&self, cache_dir: &str) -> Result<(), git2::Error> {
        git2::Repository::clone(
            &format!("{}/{}", AUR_URL, self.name),
            &format!("{}/{}", cache_dir, self.name),
        )?;
        Ok(())
    }

    fn git_pull(&self, cache_dir: &str) -> Result<bool, AURError> {
        //let repo = git2::Repository::open(&format!("{}/{}", cache_dir, self.name))?;
        let mut proc = Command::new("git")
            .current_dir(format!("{}/{}", cache_dir, self.name))
            .args(["pull"])
            .spawn()
            .unwrap();
        match proc.wait().unwrap().success() {
            true => Ok(true),
            false => {
                log::error!("Failed to pull {}", self.name);
                Err(AURError::PullFailed(self.name.clone()))
            }
        }
    }

    fn install(
        &self,
        cache_dir: &str,
        sync_deps: bool,
        rm_deps: bool,
        clean: bool,
        as_deps: bool,
        confirm: bool,
    ) -> Result<(), AURError> {
        log::trace!("PackageExt::install()");
        let mut proc = {
            let mut cmd = Command::new("makepkg");
            cmd.current_dir(format!("{}/{}", cache_dir, self.name))
                .args(["-i", "--needed"])
                .stdin(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit());
            if sync_deps {
                cmd.arg("-s");
            }
            if rm_deps {
                cmd.arg("-r");
            }
            if clean {
                cmd.arg("-c");
            }
            if as_deps {
                cmd.arg("--asdeps");
            }
            if !confirm {
                cmd.arg("--noconfirm");
            }
            log::debug!("Spawning makepkg: {:?}", cmd);
            cmd.spawn()?
        };
        match proc.wait()?.success() {
            true => Ok(()),
            false => Err(AURError::InstallFailed),
        }
    }
}

pub async fn installed_pkg_info(rpc: &raur::Handle) -> Result<HashMap<String, Package>, AURError> {
    let output = Command::new("pacman")
        .args(["-Qqm"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(AURError::PacmanFailed);
    }
    let stdout = std::str::from_utf8(&output.stdout)?;
    let pkgs = stdout.lines().collect::<Vec<_>>();
    let pkgs = HashMap::<String, Package>::from_iter(
        rpc.info(&pkgs)
            .await?
            .into_iter()
            .map(|p| (p.name.clone(), p)),
    );
    Ok(pkgs)
}

pub const AUR_URL: &str = "https://aur.archlinux.org";

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        log_lvl,
        cache_dir,
        cmd,
    } = Args::parse();
    init_fern(std::io::stdout(), log_lvl);
    log::debug!("AUR Cache Dir: {}", cache_dir);
    let rpc = raur::Handle::new();

    match cmd {
        Cmd::Install {
            as_deps,
            refresh,
            upgrade,
            sync_deps,
            rm_deps,
            clean,
            no_confirm,
            deps,
            ignore_errors,
            make_explicit,
            mut packages,
        } => {
            let mut term = Term::stdout();
            if packages.iter().any(|p| p.as_str().trim() == "*") {
                let installed_pkgs = installed_pkg_info(&rpc).await?;
                packages = Vec::from_iter(installed_pkgs.keys().cloned());
            }
            let mut deps = deps.unwrap_or_default();
            if !make_explicit {
                let mut i = 0;
                while i < packages.len() {
                    if packages[i].is_dep().unwrap_or(false) {
                        deps.push(packages.remove(i));
                        i -= 1;
                    }
                    i += 1;
                }
            }
            let exp_info = HashMap::<String, Package>::from_iter(
                rpc.info(&packages)
                    .await?
                    .into_iter()
                    .map(|p| (p.name.clone(), p)),
            );
            let mut failed = Vec::new();
            let mut succeeded = Vec::new();
            writeln!(
                &mut term,
                "Installing{}: {:?}",
                if upgrade { " / Upgrading" } else { "" },
                packages
            )?;
            if !deps.is_empty() {
                writeln!(&mut term, "Dependencies: {:?}", deps)?;
            }
            if !no_confirm {
                let stdin = std::io::stdin();
                write!(&mut term, "Continue? y/n [default: y]: ")?;
                let mut ibuf = String::new();
                match stdin.read_line(&mut ibuf).map(|_s| ibuf.trim()) {
                    Ok("y") | Ok("") => {}
                    _ => return Ok(()),
                }
            }

            let dep_info = match deps.is_empty() {
                false => HashMap::<String, Package>::from_iter(
                    rpc.info(&deps)
                        .await
                        .unwrap()
                        .into_iter()
                        .map(|p| (p.name.clone(), p)),
                ),
                true => HashMap::default(),
            };
            let mut upgrade_fn =
                |i, name, pkg: &Package| -> Result<(), Box<dyn std::error::Error>> {
                    let mut cloned = false;
                    let mut changed = false;
                    if !pkg.cached(&cache_dir).unwrap_or(false) {
                        log::info!("Cloning {}...", name);
                        pkg.git_clone(&cache_dir)?;
                        cloned = true;
                    }
                    if refresh && !cloned {
                        log::info!("Pulling {}...", name);
                        changed = pkg.git_pull(&cache_dir)?;
                    }
                    if changed || cloned || upgrade {
                        log::info!(
                            "Installing {} ({}/{})...",
                            name,
                            i,
                            dep_info.len() + exp_info.len()
                        );
                        pkg.install(
                            &cache_dir,
                            sync_deps,
                            rm_deps,
                            clean,
                            as_deps || dep_info.contains_key(name),
                            !no_confirm,
                        )?;
                        succeeded.push(name);
                    }
                    Ok(())
                };
            for (i, (name, pkg)) in dep_info.iter().chain(exp_info.iter()).enumerate() {
                let res = upgrade_fn(i, name, pkg);
                match (res, ignore_errors) {
                    (Ok(_), _) => {}
                    (Err(e), true) => {
                        log::error!("Failed to sync {}: {:?}", name, e);
                        failed.push((name.clone(), e));
                    }
                    (Err(e), false) => return Err(e),
                }
            }
            if !failed.is_empty() {
                log::error!(
                    "Failed to sync {} package{}: {:?}",
                    failed.len(),
                    if failed.len() == 1 { "" } else { "s" },
                    failed
                );
            }
            if !succeeded.is_empty() {
                log::info!(
                    "Synced {} package{}: {:?}",
                    succeeded.len(),
                    if succeeded.len() == 1 { "" } else { "s" },
                    succeeded
                );
            }
        }
        Cmd::Clean {
            only_uninstalled,
            no_confirm,
            ignore_errors,
        } => {
            let cached_pkgs = {
                let entries = fs::read_dir(&cache_dir)?
                    .filter_map(|e| {
                        let e = match e {
                            Ok(e) => e,
                            _ => return None,
                        };
                        match e.file_type() {
                            Ok(f) if f.is_dir() => e.file_name().into_string().ok(),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                let info = rpc.info(&entries).await?;
                HashMap::<String, Package>::from_iter(info.into_iter().map(|p| (p.name.clone(), p)))
            };
            let stdin = std::io::stdin();
            let mut ibuf;
            let mut term = Term::stdout();
            let mut deleted = 0;
            for (name, pkg) in cached_pkgs {
                if only_uninstalled && pkg.installed().is_ok() {
                    continue;
                }
                let dir = format!("{}/{}", cache_dir, name);
                match no_confirm {
                    true => {}
                    false => {
                        writeln!(&mut term, "Delete {}?", dir)?;
                        write!(&mut term, "y/n [default: y] > ")?;
                        ibuf = String::new();
                        match stdin.read_line(&mut ibuf).map(|_s| ibuf.trim()) {
                            Ok("y") | Ok("") => {}
                            _ => continue,
                        }
                    }
                }
                log::info!("Deleting {}", dir);
                match (std::fs::remove_dir_all(&dir), ignore_errors) {
                    (Ok(_), _) => deleted += 1,
                    (Err(e), true) => {
                        log::error!("Delete failed, skipping: {:?}", e);
                    }
                    (Err(e), false) => return Err(e.into()),
                }
            }
            writeln!(
                &mut term,
                "Deleted {} cache director{}.",
                deleted,
                if deleted == 1 { "y" } else { "ies" }
            )?;
        }
        Cmd::Query {
            field,
            keywords,
            keep_outdated,
            keep_unmaintained,
            no_url,
            sort_by,
            sort_dir,
            installed,
        } => {
            let mut installed_info = installed_pkg_info(&rpc).await?;
            let mut pkgs: &mut Vec<Package>;
            let mut installed_pkgs = Vec::new();
            let mut remote_pkgs = Vec::new();
            match installed {
                false => {
                    log::info!("Searching {:?} :: {}", field, keywords);
                    remote_pkgs = rpc.search_by(keywords, field).await?;
                    pkgs = &mut remote_pkgs;
                }
                true => {
                    installed_pkgs = installed_info.values().cloned().collect();
                    pkgs = &mut installed_pkgs;
                }
            };

            let mut skipped_outdated = 0;
            let mut skipped_unmaintained = 0;
            let mut term = Term::stdout();
            let cerr = Style::new().fg(Color::Red);
            let cname = Style::new().fg(Color::Yellow); // Fg(color::Yellow);
            let cversion = Style::new().fg(Color::Blue);
            let csubmitted = Style::new().fg(Color::Red);
            let cmodified = Style::new().fg(Color::Green);
            let curl = Style::new().fg(Color::Cyan);
            let ctotal = Style::new().fg(Color::Cyan);
            let cinstalled = Style::new().fg(Color::Cyan);
            let date_format =
                time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
            match (sort_by, sort_dir) {
                (Some(SortBy::Name), SortDir::Descending) => {
                    pkgs.sort_unstable_by(|a, b| a.name.cmp(&b.name));
                }
                (Some(SortBy::Name), SortDir::Ascending) => {
                    pkgs.sort_unstable_by(|a, b| b.name.cmp(&a.name));
                }
                (Some(SortBy::Modified), SortDir::Descending) => {
                    pkgs.sort_unstable_by(|a, b| b.last_modified.cmp(&a.last_modified));
                }
                (Some(SortBy::Modified), SortDir::Ascending) => {
                    pkgs.sort_unstable_by(|a, b| a.last_modified.cmp(&b.last_modified));
                }
                (Some(SortBy::Submitted), SortDir::Descending) => {
                    pkgs.sort_unstable_by(|a, b| b.first_submitted.cmp(&a.first_submitted));
                }
                (Some(SortBy::Submitted), SortDir::Ascending) => {
                    pkgs.sort_unstable_by(|a, b| a.first_submitted.cmp(&b.first_submitted));
                }
                (Some(SortBy::Votes), SortDir::Descending) => {
                    pkgs.sort_unstable_by(|a, b| b.num_votes.cmp(&a.num_votes));
                }
                (Some(SortBy::Votes), SortDir::Ascending) => {
                    pkgs.sort_unstable_by(|a, b| a.num_votes.cmp(&b.num_votes));
                }
                (Some(SortBy::Popularity), SortDir::Descending) => {
                    pkgs.sort_unstable_by(|a, b| {
                        b.popularity
                            .partial_cmp(&a.popularity)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                (Some(SortBy::Popularity), SortDir::Ascending) => {
                    pkgs.sort_unstable_by(|a, b| {
                        a.popularity
                            .partial_cmp(&b.popularity)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                (None, _) => {}
            }
            for pkg in pkgs.iter() {
                if !keep_outdated && pkg.out_of_date.is_some() {
                    skipped_outdated += 1;
                    continue;
                }
                if !keep_unmaintained && pkg.maintainer.is_none() {
                    skipped_unmaintained += 1;
                    continue;
                }
                write!(
                    &mut term,
                    "{} : {}{} : {} : {} : ^{} : {:.2}%",
                    cname.apply_to(&pkg.name),
                    cversion.apply_to(&pkg.version),
                    if installed_info.contains_key(&pkg.name) {
                        let installed = installed_info[&pkg.name].installed()?;
                        format!(" <{}>", cinstalled.apply_to(installed))
                    } else {
                        String::new()
                    },
                    csubmitted.apply_to(
                        time::OffsetDateTime::from_unix_timestamp(pkg.first_submitted)?
                            .format(&date_format)?
                    ),
                    cmodified.apply_to(
                        time::OffsetDateTime::from_unix_timestamp(pkg.last_modified)?
                            .format(&date_format)?
                    ),
                    ctotal.apply_to(&pkg.num_votes),
                    ctotal.apply_to(&pkg.popularity)
                )?;
                if !no_url {
                    write!(
                        &mut term,
                        "\n\t<< {} ",
                        curl.apply_to(format!("{}/packages/{}", AUR_URL, &pkg.name))
                    )?;
                    if let Some(url) = pkg.url.as_ref() {
                        write!(&mut term, "<- {} ", curl.apply_to(url))?;
                    }
                    write!(&mut term, ">>")?;
                }
                if pkg.out_of_date.is_some() || pkg.maintainer.is_none() {
                    write!(&mut term, "\n\t{0}{0} ", cerr.apply_to('⚠'))?;
                    if let Some(ood) = pkg.out_of_date {
                        write!(
                            &mut term,
                            "Out of date as of {} ",
                            time::OffsetDateTime::from_unix_timestamp(ood)?.format(&date_format)?
                        )?;
                    }
                    if pkg.maintainer.is_none() {
                        write!(&mut term, "No Maintainer ")?;
                    }
                }
                if let Some(d) = &pkg.description {
                    write!(&mut term, "\n\t-- {}", d)?;
                }
                writeln!(&mut term)?;
            }
            let displayed_total = pkgs.len() - skipped_unmaintained - skipped_outdated;
            writeln!(
                &mut term,
                "Displaying {} package{}.",
                ctotal.apply_to(displayed_total),
                if displayed_total == 1 { "" } else { "s" }
            )?;
            if skipped_unmaintained > 0 || skipped_outdated > 0 {
                writeln!(
                    &mut term,
                    "Skipped {} outdated and {} unmaintained packages.",
                    ctotal.apply_to(skipped_outdated),
                    ctotal.apply_to(skipped_unmaintained)
                )?;
            }
        }
    }
    Ok(())
}
