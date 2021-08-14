use std::collections::HashMap;

use super::version_compare;
use super::UpdateChecker;
use crate::must_have;
use anyhow::{anyhow, Result};
use log::debug;
use regex::Regex;
use reqwest::blocking::Client;

pub(crate) struct HTMLChecker {
    url: String,
    pattern: String,
}

impl UpdateChecker for HTMLChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(HTMLChecker {
            url: must_have!(config, "url", "HTML URL")?.to_string(),
            pattern: must_have!(config, "pattern", "Regex pattern for matching versions")?
                .to_string(),
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        let resp = client.get(&self.url).send()?;
        if let Some(len) = resp.content_length() {
            if len > 10 * 1024 * 1024 {
                // 10 MB
                return Err(anyhow!("HTML body too large"));
            }
        }
        resp.error_for_status_ref()?;
        let body = resp.text()?;
        let pattern = Regex::new(&self.pattern)?;
        let matches = pattern.captures_iter(&body);
        let mut versions = Vec::new();
        versions.reserve(10);
        for m in matches {
            versions.push(
                m.get(1)
                    .ok_or_else(|| anyhow!("Pattern did not capture anything."))?
                    .as_str(),
            );
        }
        if versions.len() < 1 {
            return Err(anyhow!("No version matches the pattern."));
        } else if versions.len() == 1 {
            return Ok(versions[0].to_string());
        }
        debug!("matched tags: {:?}", versions);

        versions.sort_unstable_by(|a, b| version_compare(a, b));

        return Ok(versions.last().unwrap().to_string());
    }
}

#[test]
fn test_check_anitya() {
    let mut options = HashMap::new();
    options.insert(
        "url".to_string(),
        "https://repo.aosc.io/misc/l10n/".to_string(),
    );
    options.insert("pattern".to_string(), "zh_CN_l10n_(.+?)\\.pdf".to_string());
    let client = Client::new();
    let checker = HTMLChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
