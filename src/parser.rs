use anyhow::{anyhow, Result};
use log::{info, warn};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};
use winnow::{
    ascii::alphanumeric1,
    combinator::{alt, repeat, separated_pair, terminated},
    token::{tag, take_until0},
    PResult, Parser,
};

type Context = HashMap<String, String>;

const CONFIG_SEPARATOR: &str = "::";

fn take_type<'a>(input: &mut &'a str) -> PResult<&'a str> {
    take_until0(CONFIG_SEPARATOR).parse_next(input)
}

fn kv_key_inner(input: &mut &str) -> PResult<()> {
    repeat(1.., alt((alphanumeric1, tag("_")))).parse_next(input)
}

fn kv_key<'a>(input: &mut &'a str) -> PResult<&'a str> {
    kv_key_inner.recognize().parse_next(input)
}

fn kv_pair<'a>(input: &mut &'a str) -> PResult<(&'a str, &'a str)> {
    separated_pair(kv_key, tag("="), take_until0(";")).parse_next(input)
}

fn kv_pairs<'a>(input: &mut &'a str) -> PResult<Vec<(&'a str, &'a str)>> {
    repeat(1.., terminated(kv_pair, tag(";"))).parse_next(input)
}

fn config_line<'a>(input: &mut &'a str) -> PResult<(&'a str, Vec<(&'a str, &'a str)>)> {
    separated_pair(take_type, tag(CONFIG_SEPARATOR), kv_pairs).parse_next(input)
}

pub(crate) fn parse_spec<P: AsRef<Path>>(spec: P) -> Result<Context> {
    let mut f = File::open(spec.as_ref())?;
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    let mut context = HashMap::new();

    abbs_meta_apml::parse(&contents, &mut context)?;

    Ok(context)
}

pub(crate) fn parse_check_update(content: &mut &str) -> Result<Context> {
    let parsed = config_line(content).map_err(|err| anyhow!("Invalid config line: {}", err))?;
    let mut context = HashMap::new();
    let config = parsed.1;
    context.insert("type".to_string(), content.to_string());

    for (k, v) in config {
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
pub(crate) fn expand_package_list<P: AsRef<Path>, I: IntoIterator<Item = P>>(
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
    let test = &mut "test::1";
    let res = take_type(test);

    assert_eq!(res, Ok("test"));
    assert_eq!(test, &mut "::1");
}

#[test]
fn test_kv_key() {
    let test = &mut "a_b123";
    let res = kv_key(test);
    assert_eq!(res, Ok("a_b123"));
    assert_eq!(test, &mut "");
}

#[test]
fn test_kv() {
    let test = &mut "a=b;";
    let res = kv_pair(test);
    assert_eq!(res, Ok(("a", "b")));
    assert_eq!(test, &mut ";")
}

#[test]
fn test_kv_pairs() {
    let test = &mut "a=b;b=d;";
    let res = kv_pairs(test);

    assert_eq!(res, Ok(vec![("a", "b"), ("b", "d")]));
    assert_eq!(test, &mut "");
}
