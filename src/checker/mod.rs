use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use std::{cmp::Ordering, collections::HashMap};
use version_compare::{comp_op::CompOp, VersionCompare};

mod anitya;
mod github;
mod gitlab;
mod gitweb;
mod git;
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

#[inline]
pub(crate) fn version_compare(a: &str, b: &str) -> Ordering {
    if let Ok(ret) = VersionCompare::compare(a, b) {
        match ret {
            CompOp::Eq => Ordering::Equal,
            CompOp::Lt => Ordering::Less,
            CompOp::Gt => Ordering::Greater,
            _ => a.cmp(&b),
        }
    } else {
        a.cmp(&b)
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

    Ok(checker.check(client)?)
}
