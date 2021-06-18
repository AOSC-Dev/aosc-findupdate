use std::collections::HashMap;

use super::{extract_versions, version_compare, UpdateChecker};
use crate::must_have;
use anyhow::{anyhow, Result};
use kuchiki::traits::*;
use reqwest::blocking::Client;

pub(crate) struct GitWebChecker {
    url: String,
    pattern: Option<String>,
}

impl UpdateChecker for GitWebChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(GitWebChecker {
            url: must_have!(config, "url", "GitWeb project URL")?.to_string(),
            pattern: config.get("pattern").map(|s| s.clone()),
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        let resp = client.get(&format!("{}/tags", self.url)).send()?;
        if let Some(len) = resp.content_length() {
            if len > 10 * 1024 * 1024 {
                // 10 MB
                return Err(anyhow!("HTML body too large"));
            }
        }
        let body = resp.text()?;
        let document = kuchiki::parse_html().one(body.as_str());
        let mut versions = Vec::new();

        for m in document
            .select(".name")
            .or_else(|_| Err(anyhow!("HTML selector error: class 'name' not found.")))?
        {
            let node = m.as_node();
            versions.push(node.text_contents());
        }

        if let Some(pattern) = &self.pattern {
            versions = extract_versions(pattern, &versions)?;
        }

        if versions.len() < 1 {
            return Err(anyhow!("No tags found."));
        } else if versions.len() == 1 {
            return Ok(versions[0].to_string());
        }

        versions.sort_unstable_by(|a, b| version_compare(a, b));

        return Ok(versions.last().unwrap().to_string());
    }
}

#[test]
fn test_0ad() {
    let mut options = HashMap::new();
    options.insert("url".to_string(), "https://repo.or.cz/0ad.git".to_string());
    options.insert("pattern".to_string(), "^[^b]+$".to_string());
    let client = Client::new();
    let checker = GitWebChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
