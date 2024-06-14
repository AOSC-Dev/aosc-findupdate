use std::collections::HashMap;

use super::{extract_versions, version_compare, UpdateChecker};
use crate::must_have;
use anyhow::{anyhow, Result};
use log::debug;
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use sailfish::TemplateOnce;
use serde::{Deserialize, Serialize};

const API_ENDPOINT: &str = "https://api.github.com/";

#[derive(TemplateOnce)]
#[template(path = "github.stpl")]
struct GitHubQuery {
    name: String,
    owner: String,
}

#[derive(Serialize)]
struct GitHubRequest {
    query: String,
}

#[derive(Deserialize)]
struct GitHubTagData {
    name: String,
}

#[derive(Deserialize)]
struct GitHubRef {
    nodes: Vec<GitHubTagData>,
}

#[derive(Deserialize)]
struct GitHubRepo {
    refs: GitHubRef,
}

#[derive(Deserialize)]
struct GitHubResponseInner {
    repository: GitHubRepo,
}

#[derive(Deserialize)]
struct GitHubResponse {
    data: GitHubResponseInner,
}

#[derive(Deserialize, Debug)]
struct GithubCommitRest {
    sha: String,
}

pub(crate) struct GitHubChecker {
    repo: String,
    pattern: Option<String>,
    sort_version: bool,
    branch: Option<String>,
}

impl UpdateChecker for GitHubChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker,
    {
        let repo = must_have!(config, "repo", "Repository slug")?.to_string();
        let pattern = config.get("pattern").cloned();
        let branch = config.get("branch").cloned();
        let sort_version = config
            .get("sort_version")
            .map(|s| s == "true")
            .unwrap_or(false);

        Ok(GitHubChecker {
            repo,
            pattern,
            sort_version,
            branch,
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        if let Some(branch) = &self.branch {
            self.check_rev(client, branch)
        } else {
            self.check_tags(client)
        }
    }
}

impl GitHubChecker {
    fn check_tags(&self, client: &Client) -> Result<String> {
        let mut slug = self.repo.splitn(2, '/');
        let query = GitHubQuery {
            owner: slug
                .next()
                .ok_or_else(|| anyhow!("Repository owner missing"))?
                .to_string(),
            name: slug
                .next()
                .ok_or_else(|| anyhow!("Repository name missing"))?
                .to_string(),
        }
        .render_once()?;
        let mut builder = client
            .post(format!("{}graphql", API_ENDPOINT))
            .header(USER_AGENT, "AOSCFindUpdate/0.1.0");
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            builder = builder.header(AUTHORIZATION, format!("token {}", token));
        } else {
            return Err(anyhow!("GitHub checker requires authentication! Please set GITHUB_TOKEN environment variable."));
        }
        let resp = builder.json(&GitHubRequest { query }).send()?;
        resp.error_for_status_ref()?;
        let payload: GitHubResponse = resp.json()?;
        let mut payload = payload
            .data
            .repository
            .refs
            .nodes
            .into_iter()
            .map(|node| node.name)
            .collect::<Vec<_>>();
        debug!("returned tags: {:?}", payload);
        if let Some(pattern) = &self.pattern {
            payload = extract_versions(pattern, &payload)?;
        }
        debug!("after filter: {:?}", payload);
        if payload.is_empty() {
            return Err(anyhow!("GitHub didn't return any tags!"));
        }
        if self.sort_version {
            payload.sort_unstable_by(|b, a| version_compare(a, b));
        }

        Ok(payload.first().unwrap().clone())
    }

    fn check_rev(&self, client: &Client, branch: &str) -> Result<String> {
        let mut slug = self.repo.splitn(2, '/');
        let owner = slug
            .next()
            .ok_or_else(|| anyhow!("Repository owner missing"))?;

        let repo = slug
            .next()
            .ok_or_else(|| anyhow!("Repository name missing"))?;

        let mut builder = client
            .get(format!(
                "https://api.github.com/repos/{}/{}/commits",
                owner, repo
            ))
            .header(USER_AGENT, "AOSCFindUpdate/0.1.0");

        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            builder = builder.header(AUTHORIZATION, format!("token {}", token));
        } else {
            return Err(anyhow!("GitHub checker requires authentication! Please set GITHUB_TOKEN environment variable."));
        }

        let resp = builder
            .query(&[("sha", branch), ("per_page", "1")])
            .send()?;
        resp.error_for_status_ref()?;

        let res = resp.json::<Vec<GithubCommitRest>>()?;
        let res = res
            .first()
            .ok_or_else(|| anyhow!("Repo commits is empty"))?;

        Ok(res.sha.to_string())
    }
}

#[test]
fn test_github() {
    let mut options = HashMap::new();
    options.insert("repo".to_string(), "AOSC-Dev/ciel-rs".to_string());
    let client = Client::new();
    let checker = GitHubChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}

#[test]
fn test_github_with_branch() {
    let mut options = HashMap::new();
    options.insert("repo".to_string(), "AOSC-Dev/ciel-rs".to_string());
    options.insert("branch".to_string(), "master".to_string());
    let client = Client::new();
    let checker = GitHubChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
