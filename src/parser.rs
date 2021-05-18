use anyhow::{anyhow, Result};
use log::{info, warn};
use nom::{
    bytes::complete::{tag, take_until},
    character::complete::alphanumeric1,
    multi::many1,
    sequence::{separated_pair, terminated},
    IResult,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

type Context = HashMap<String, String>;

const CONFIG_SEPARATOR: &str = "::";

fn take_type(input: &str) -> IResult<&str, &str> {
    take_until(CONFIG_SEPARATOR)(input)
}

fn kv_pair(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(alphanumeric1, tag("="), take_until(";"))(input)
}

fn kv_pairs(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
    many1(terminated(kv_pair, tag(";")))(input)
}

fn config_line(input: &str) -> IResult<&str, (&str, Vec<(&str, &str)>)> {
    separated_pair(take_type, tag(CONFIG_SEPARATOR), kv_pairs)(input)
}

pub(crate) fn parse_spec<P: AsRef<Path>>(spec: P) -> Result<Context> {
    let mut f = File::open(spec.as_ref())?;
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    let mut context = HashMap::new();

    abbs_meta_apml::parse(&contents, &mut context)?;

    Ok(context)
}

pub(crate) fn parse_check_update(content: &str) -> Result<Context> {
    let parsed = config_line(content).map_err(|err| anyhow!("Invalid config line: {}", err))?;
    let mut context = HashMap::new();
    let config = parsed.1;
    context.insert("type".to_string(), config.0.to_string());

    for (k, v) in config.1 {
        context.insert(k.to_string(), v.to_string());
    }

    Ok(context)
}

// copied from ciel

fn read_package_list<P: AsRef<Path>>(filename: P, depth: usize) -> Result<Vec<String>> {
    if depth > 32 {
        return Err(anyhow!(
            "Nested group exceeded 32 levels! Potential infinite loop."
        ));
    }
    let f = File::open(filename)?;
    let reader = BufReader::new(f);
    let mut results = Vec::new();
    for line in reader.lines() {
        let line = line?;
        // skip comment
        if line.starts_with('#') {
            continue;
        }
        // trim whitespace
        let trimmed = line.trim();
        // process nested groups
        if trimmed.starts_with("groups/") {
            let path = Path::new(".").join(trimmed);
            let nested = read_package_list(&path, depth + 1)?;
            results.extend(nested);
            continue;
        }
        results.push(trimmed.to_owned());
    }

    Ok(results)
}

/// Expand the packages list to an array of packages
pub(crate) fn expand_package_list<'a, P: AsRef<Path>, I: IntoIterator<Item = P>>(
    packages: I,
) -> Vec<String> {
    let mut expanded = Vec::new();
    for package in packages {
        match read_package_list(package.as_ref(), 0) {
            Ok(list) => {
                info!(
                    "Read {} packages from {}",
                    list.len(),
                    package.as_ref().display()
                );
                expanded.extend(list);
            }
            Err(e) => {
                warn!(
                    "Unable to read package group `{}`: {}",
                    package.as_ref().display(),
                    e
                );
            }
        }
    }

    expanded
}

#[test]
fn test_take_type() {
    let test = "test::1";
    assert_eq!(take_type(test), Ok(("::1", "test")));
}

#[test]
fn test_kv() {
    let test = "a=b;";
    assert_eq!(kv_pair(test), Ok((";", ("a", "b"))));
}

#[test]
fn test_kv_pairs() {
    let test = "a=b;b=d;";
    assert_eq!(kv_pairs(test), Ok(("", vec![("a", "b"), ("b", "d")])));
}
