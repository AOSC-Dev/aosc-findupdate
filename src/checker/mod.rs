use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::blocking::Client;
use std::{cmp::Ordering, collections::HashMap};
use version_compare::{compare, Cmp};

mod anitya;
mod git;
mod github;
mod gitlab;
mod gitweb;
mod html;

#[macro_export]
macro_rules! must_have {
    ($c:ident, $i:expr, $d:expr) => {
        $c.get($i)
            .ok_or_else(|| anyhow!(concat!("Please specify ", $d, "!")))
    };
}

macro_rules! use_this {
    ($c:ty, $config:ident) => {
        Box::new(<$c>::new($config)?)
    };
}

pub(crate) fn extract_versions<S: AsRef<str>>(
    pattern: &str,
    collection: &[S],
) -> Result<Vec<String>> {
    let regex = Regex::new(pattern)?;
    let results = if regex.captures_len() > 1 {
        collection
            .iter()
            .filter_map(|x| {
                regex
                    .captures(x.as_ref())
                    .and_then(|x| x.get(1))
                    .map(|x| x.as_str().to_string())
            })
            .collect()
    } else {
        collection
            .iter()
            .filter_map(|x| regex.is_match(x.as_ref()).then(|| x.as_ref().to_string()))
            .collect()
    };

    Ok(results)
}

#[inline]
pub(crate) fn version_compare(a: &str, b: &str) -> Ordering {
    if let Ok(ret) = compare(a, b) {
        match ret {
            Cmp::Eq => Ordering::Equal,
            Cmp::Lt => Ordering::Less,
            Cmp::Gt => Ordering::Greater,
            _ => a.cmp(b),
        }
    } else {
        a.cmp(b)
    }
}

/// Abstraction for an update checker
pub trait UpdateChecker {
    /// Create a new update checker instance with specified options
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker;
    /// Check the update
    fn check(&self, client: &Client) -> Result<String>;
}

pub fn check_update(config: &HashMap<String, String>, client: &Client) -> Result<String> {
    let ty = config
        .get("type")
        .ok_or_else(|| anyhow!("Upstream type not specified."))?
        .as_str();
    let checker: Result<Box<dyn UpdateChecker>> = match ty {
        "anitya" => Ok(use_this!(anitya::AnityaChecker, config)),
        "github" => Ok(use_this!(github::GitHubChecker, config)),
        "gitlab" => Ok(use_this!(gitlab::GitLabChecker, config)),
        "gitweb" => Ok(use_this!(gitweb::GitWebChecker, config)),
        "git" => Ok(use_this!(git::GitChecker, config)),
        "html" => Ok(use_this!(html::HTMLChecker, config)),
        _ => Err(anyhow!("Unknown type")),
    };
    let checker = checker?;

    checker.check(client)
}
