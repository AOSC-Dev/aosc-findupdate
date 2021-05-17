use std::collections::HashMap;

use super::version_compare;
use super::UpdateChecker;
use crate::must_have;
use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;

const API_ENDPOINT: &str = "https://api.github.com/";

#[derive(Deserialize)]
struct GitHubData {
    name: String,
}

pub(crate) struct GitHubChecker {
    repo: String,
    pattern: Option<String>,
    sort_version: bool,
}

impl UpdateChecker for GitHubChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker,
    {
        let repo = must_have!(config, "repo", "Repository slug")?.to_string();
        let pattern = config.get("pattern").map(|s| s.clone());
        let sort_version = config
            .get("sort_version")
            .map(|s| s == "true")
            .unwrap_or(false);

        Ok(GitHubChecker {
            repo,
            pattern,
            sort_version,
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        let resp = client
            .get(&format!("{}repos/{}/", API_ENDPOINT, self.repo))
            .send()?;
        let mut payload: Vec<GitHubData> = resp.json()?;
        if let Some(pattern) = &self.pattern {
            let regex = Regex::new(&pattern)?;
            payload = payload
                .into_iter()
                .filter(|x| regex.is_match(&x.name))
                .collect();
        }
        if payload.len() < 1 {
            return Err(anyhow!("GitHub didn't return any tags!"));
        }
        if self.sort_version {
            payload.sort_unstable_by(|b, a| version_compare(&a.name, &b.name));
        }

        Ok(payload.first().unwrap().name.clone())
    }
}
