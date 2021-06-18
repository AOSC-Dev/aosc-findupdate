use std::collections::HashMap;

use super::{extract_versions, version_compare, UpdateChecker};
use crate::must_have;
use anyhow::{anyhow, Result};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use reqwest::blocking::Client;
use serde::Deserialize;

const API_ENDPOINT: &str = "https://gitlab.com";

#[derive(Deserialize)]
struct GitLabData {
    name: String,
}

pub(crate) struct GitLabChecker {
    instance: String,
    repo: String,
    pattern: Option<String>,
    sort_version: bool,
}

impl UpdateChecker for GitLabChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker,
    {
        let repo = must_have!(config, "repo", "Repository slug or Project ID")?.to_string();
        let instance = config
            .get("instance")
            .map(|s| s.clone())
            .unwrap_or_else(|| API_ENDPOINT.to_string());
        let pattern = config.get("pattern").map(|s| s.clone());
        let sort_version = config
            .get("sort_version")
            .map(|s| s == "true")
            .unwrap_or(false);

        Ok(GitLabChecker {
            instance,
            repo,
            pattern,
            sort_version,
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        let resp = client
            .get(&format!(
                "{}/api/v4/projects/{}/repository/tags",
                self.instance,
                percent_encode(self.repo.as_bytes(), NON_ALPHANUMERIC)
            ))
            .send()?;
        let payload: Vec<GitLabData> = resp.json()?;
        let mut payload = payload.into_iter().map(|x| x.name).collect::<Vec<_>>();
        if let Some(pattern) = &self.pattern {
            payload = extract_versions(pattern, &payload)?;
        }
        if payload.len() < 1 {
            return Err(anyhow!(
                "GitLab ({}) didn't return any tags!",
                self.instance
            ));
        }
        if self.sort_version {
            payload.sort_unstable_by(|b, a| version_compare(&a, &b));
        }

        Ok(payload.first().unwrap().clone())
    }
}

#[test]
fn test_gnome() {
    let mut options = HashMap::new();
    options.insert("repo".to_string(), "GNOME/fractal".to_string());
    options.insert(
        "instance".to_string(),
        "https://gitlab.gnome.org".to_string(),
    );
    let client = Client::new();
    let checker = GitLabChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
