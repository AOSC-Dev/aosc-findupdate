use aho_corasick::AhoCorasickBuilder;
use anyhow::{anyhow, Result};
use log::info;
use owo_colors::colored::*;
use rayon::prelude::*;
use regex::Regex;
use reqwest::blocking::Client;
use std::{
    borrow::Cow,
    collections::HashMap,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use version_compare::{CompOp, VersionCompare};

mod checker;
mod cli;
mod parser;

const VCS_VERSION_NUMBERS: &[&str] = &["+git", "+hg", "+svn", "+bzr"];

#[derive(Debug)]
struct CheckerResult {
    name: String,
    before: String,
    after: String,
    warnings: Vec<String>,
}

fn collect_spec(dir: &Path) -> Result<Vec<PathBuf>> {
    let walker = walkdir::WalkDir::new(dir).max_depth(3);
    let result = walker
        .into_iter()
        .filter_map(|x| {
            let entry = x.ok()?;
            if entry.file_name() == "spec" {
                Some(PathBuf::from(entry.path()))
            } else {
                None
            }
        })
        .collect();

    Ok(result)
}

fn normalize_name<'a>(path: &'a Path) -> Cow<str> {
    let p = path.strip_prefix("./").unwrap_or(path);
    let p = p.parent().unwrap_or(path);

    p.to_string_lossy()
}

fn update_version<P: AsRef<Path>>(new: &str, spec: P) -> Result<String> {
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(spec.as_ref())?;
    let mut content = String::new();
    f.read_to_string(&mut content)?;
    let replace = Regex::new("VER=.+").unwrap();
    let replace_rel = Regex::new("REL=.+").unwrap();
    let replaced = replace.replace(&content, format!("VER={}", new));
    let replaced = replace_rel.replace(&replaced, "");
    f.seek(SeekFrom::Start(0))?;
    let bytes = replaced.as_bytes();
    f.write_all(bytes)?;
    f.set_len(bytes.len() as u64)?;

    Ok(replaced.to_string())
}

fn validate_urls(a: &HashMap<String, String>, b: &HashMap<String, String>) -> bool {
    for (key, value) in a.iter() {
        if !key.starts_with("SRCS") {
            continue;
        }
        if let Some(other) = b.get(key) {
            let a_split = value.split_ascii_whitespace();
            let b_split = other.split_ascii_whitespace();
            for (old, new) in a_split.zip(b_split) {
                if old == new {
                    return true;
                }
            }
        }
    }

    false
}

fn check_update_worker<P: AsRef<Path>>(client: &Client, spec: P) -> Result<CheckerResult> {
    let s = parser::parse_spec(spec.as_ref())?;
    let current_version = s
        .get("VER")
        .ok_or_else(|| anyhow!("{}: 'VER' field is missing!", spec.as_ref().display()))?;
    let current_version = current_version.trim();
    let config_line = s.get("CHKUPDATE").ok_or_else(|| {
        anyhow!(
            "{}: 'CHKUPDATE' field is missing, cannot continue!",
            spec.as_ref().display()
        )
    })?;
    let config_line = config_line.to_owned() + ";"; // compensate for the parser quirk
    let config = parser::parse_check_update(&config_line)?;
    let new_version = checker::check_update(&config, client)?;
    let new_version = new_version.trim();
    let new_version = new_version.strip_prefix("v").unwrap_or(new_version);
    let name = normalize_name(spec.as_ref()).to_string();
    let mut warnings = Vec::new();
    if current_version == new_version {
        return Ok(CheckerResult {
            name,
            warnings,
            before: current_version.to_string(),
            after: new_version.to_string(),
        });
    }
    let snapshot_version = AhoCorasickBuilder::new().build(VCS_VERSION_NUMBERS);
    if current_version.contains("+") {
        warnings.push(format!("Compound version number '{}'", current_version));
        if let Some(version) = snapshot_version.find(current_version) {
            warnings.push(format!(
                "Version number indicates a snapshot ({}) is used",
                VCS_VERSION_NUMBERS[version.pattern()]
            ))
        }
    }
    if let Ok(ret) = VersionCompare::compare(current_version, new_version) {
        if ret == CompOp::Gt {
            warnings.push(format!(
                "Possible downgrade from the current version ({} -> {})",
                current_version, new_version
            ));
        }
    } else {
        warnings.push(format!(
            "Versions not comparable: `{}` and `{}`",
            current_version, new_version
        ));
    }
    let modified = update_version(new_version, spec.as_ref())?;
    let mut new_ctx = HashMap::new();
    match abbs_meta_apml::parse(&modified, &mut new_ctx) {
        Ok(_) => {
            if validate_urls(&s, &new_ctx) {
                warnings.push(format!("Hardcoded URLs detected."));
            }
        }
        Err(err) => {
            warnings.push(format!("Modified spec is broken: {}", err));
        }
    }

    Ok(CheckerResult {
        name,
        warnings,
        before: current_version.to_string(),
        after: new_version.to_string(),
    })
}

fn print_results(results: &[Result<CheckerResult>]) {
    println!("The following packages were updated:");
    println!("Name\t\t\t\tVersion\t\t\t\tIssues");
    for result in results {
        if let Ok(result) = result {
            if result.before == result.after {
                continue;
            }
            println!(
                "{}\t\t{} -> {}\t\t{}",
                result.name.cyan(),
                result.before.red(),
                result.after.green(),
                result.warnings.join(";\t").yellow()
            );
        }
    }
    println!("\nErrors:");
    for result in results {
        if let Err(e) = result {
            println!("{}", e.bold());
        }
    }
}

fn main() {
    let args = cli::build_cli().get_matches();
    env_logger::init();
    let mut pattern = None;
    if let Some(p) = args.value_of("INCLUDE") {
        pattern = Some(Regex::new(p).unwrap());
    }
    let workdir = if let Some(d) = args.value_of("DIR") {
        Path::new(d).canonicalize().unwrap()
    } else {
        Path::new(".").canonicalize().unwrap()
    };

    let mut files = if let Some(list) = args.value_of("FILE") {
        let path = Path::new(list).canonicalize().unwrap();
        std::env::set_current_dir(workdir).expect("Failed to set current directory");
        let list = parser::expand_package_list(&[&path]);
        list.into_iter()
            .map(|x| Path::new(&x).join("spec"))
            .collect()
    } else {
        std::env::set_current_dir(workdir).expect("Failed to set current directory");
        collect_spec(Path::new(".")).unwrap()
    };

    if let Some(pattern) = pattern {
        files = files
            .into_iter()
            .filter(|x| {
                if let Some(name) = x.parent().map(|p| p.to_string_lossy()) {
                    pattern.is_match(&name)
                } else {
                    false
                }
            })
            .collect();
    }

    let total = files.len();
    info!("Checking updates for {} packages ...", total);
    let current = Arc::new(AtomicUsize::new(1));

    let results: Vec<_> = files
        .par_iter()
        .map_init(
            || Client::new(),
            |c, f| {
                let name = normalize_name(f);
                let current = current.fetch_add(1, Ordering::SeqCst);
                info!("[{}/{}] Checking {} ...", current, total, &name);
                check_update_worker(c, f).map_err(|e| anyhow!("{}: {:?}", name, e))
            },
        )
        .collect();

    print_results(&results);
}
