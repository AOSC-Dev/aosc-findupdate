use anyhow::{anyhow, Result};
use log::info;
use rayon::prelude::*;
use regex::Regex;
use reqwest::blocking::Client;
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

mod checker;
mod cli;
mod parser;

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
    let replaced = replace.replace(&content, format!("VER={}", new));
    f.seek(SeekFrom::Start(0))?;
    f.write_all(replaced.as_bytes())?;

    Ok(content)
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
    if current_version.contains("+") {
        warnings.push(format!("Compound version number '{}'", current_version))
    }
    if checker::version_compare(current_version, new_version) == Ordering::Greater {
        warnings.push(format!(
            "Possible downgrade from the current version ({} -> {})",
            current_version, new_version
        ));
    }
    let modified = update_version(new_version, spec.as_ref())?;
    let mut new_content = HashMap::new();
    match abbs_meta_apml::parse(&modified, &mut new_content) {
        Ok(_) => {
            if validate_urls(&s, &new_content) {
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
    for result in results {
        if let Ok(result) = result {
            if result.before == result.after {
                continue;
            }
            println!(
                "{}\t\t{} -> {}\t\t{}",
                result.name,
                result.before,
                result.after,
                result.warnings.join("\t")
            );
        }
    }
    println!("\nErrors:");
    for result in results {
        if let Err(e) = result {
            println!("{}", e);
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

    info!("Checking updates for {} packages ...", files.len());

    let results: Vec<_> = files
        .par_iter()
        .map_init(
            || Client::new(),
            |c, f| {
                let name = normalize_name(f);
                info!("Checking {} ...", &name);
                check_update_worker(c, f)
            },
        )
        .collect();

    print_results(&results);
}
