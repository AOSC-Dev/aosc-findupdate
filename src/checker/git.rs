use std::collections::HashMap;

use super::{extract_versions, version_compare, UpdateChecker};
use crate::must_have;
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use winnow::{
    ascii::{multispace1, not_line_ending, space1},
    combinator::{repeat, separated_pair, terminated},
    stream::AsChar,
    token::take_while,
    PResult, Parser,
};

const SIMULATED_GIT_VERSION: &str = "2.31.1";

// parser-combinators for parsing Git on-wire format
fn first_tuple<'a>(input: &mut &'a [u8]) -> PResult<&'a [u8]> {
    take_while(1.., |c: u8| c.is_hex_digit() || c == b'#').parse_next(input)
}

fn kv_pair<'a>(input: &mut &'a [u8]) -> PResult<(&'a [u8], &'a [u8])> {
    separated_pair(first_tuple, space1, not_line_ending).parse_next(input)
}

fn single_line<'a>(input: &mut &'a [u8]) -> PResult<(&'a [u8], &'a [u8])> {
    terminated(kv_pair, multispace1).parse_next(input)
}

fn parse_git_manifest<'a>(input: &mut &'a [u8]) -> PResult<Vec<(&'a [u8], &'a [u8])>> {
    repeat(1.., single_line).parse_next(input)
}
// end of parser-combinators

fn collect_git_tags<'a>(input: &mut &'a [u8]) -> Result<Vec<&'a str>> {
    let tuples = parse_git_manifest(input).map_err(|e| anyhow!("Parser error: {:?}", e))?;
    let tags: Vec<_> = tuples
        .iter()
        .filter_map(|x| {
            if x.1.ends_with(&b"^{}"[..]) {
                None
            } else if let Some(name) = x.1.strip_prefix(&b"refs/tags/"[..]) {
                std::str::from_utf8(name).ok()
            } else {
                None
            }
        })
        .collect();

    Ok(tags)
}

pub(crate) struct GitChecker {
    url: String,
    pattern: Option<String>,
}

impl UpdateChecker for GitChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker,
    {
        let url = must_have!(config, "url", "Repository URL")?.to_string();
        let pattern = config.get("pattern").cloned();

        Ok(GitChecker { url, pattern })
    }

    fn check(&self, client: &Client) -> Result<String> {
        // this check method uses a fake Git client implementation
        let resp = client
            .get(&format!("{}/info/refs?service=git-upload-pack", self.url,))
            .header(USER_AGENT, format!("git/{}", SIMULATED_GIT_VERSION))
            .header("git-protocol", "version=2")
            .send()?;
        resp.error_for_status_ref()?;
        let body = resp.bytes()?;
        let mut tags = collect_git_tags(&mut body.to_vec().as_ref())?
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        if let Some(pattern) = &self.pattern {
            tags = extract_versions(pattern, &tags)?;
        }
        if tags.is_empty() {
            return Err(anyhow!("Git ({}) didn't return any tags!", self.url));
        }
        tags.sort_unstable_by(|b, a| version_compare(a, b));

        Ok(tags.first().unwrap().to_string())
    }
}

#[test]
fn first_tuple_test() {
    let test = &mut &b"001e# "[..];
    assert_eq!(first_tuple(test), Ok(&b"001e#"[..]));
    assert_eq!(test, &mut &b" "[..]);
}

#[test]
fn kv_test() {
    // blob descriptor
    let test = &mut &b"003fdb358a2993be0e0aa3864ed3290105dd4a544c35 refs/heads/avx512\n"[..];
    assert_eq!(
        kv_pair(test),
        Ok((
            &b"003fdb358a2993be0e0aa3864ed3290105dd4a544c35"[..],
            &b"refs/heads/avx512"[..]
        ))
    );
    assert_eq!(test, &mut &b"\n"[..]);
    // service descriptor
    let test = &mut &b"001e# service=git-upload-pack\n"[..];
    assert_eq!(
        kv_pair(test),
        Ok((&b"001e#"[..], &b"service=git-upload-pack"[..]))
    );
    assert_eq!(test, &mut &b"\n"[..]);
    // capability descriptor
    let test = &mut &b"000000fe68e3802b238b964900acac9422a70e295482243f HEAD\x00multi_ack no-done symref=HEAD:refs/heads/master agent=git/2.11.4.GIT\n"[..];
    assert_eq!(
        kv_pair(test),
        Ok((
            &b"000000fe68e3802b238b964900acac9422a70e295482243f"[..],
            &b"HEAD\x00multi_ack no-done symref=HEAD:refs/heads/master agent=git/2.11.4.GIT"[..]
        ))
    );
    assert_eq!(test, &mut &b"\n"[..],);
}

#[test]
fn test_multiline() {
    let test = &mut &b"01234abc heads\n12345bcd tags\n"[..];
    assert_eq!(
        parse_git_manifest(test),
        Ok(vec![
            (&b"01234abc"[..], &b"heads"[..]),
            (&b"12345bcd"[..], &b"tags"[..]),
        ])
    );
    assert_eq!(test, &mut &b""[..]);
    // with caps and trailer
    let test = &mut &b"001e# service=git-upload-pack\n01234abc heads\n12345bcd tags\n0000"[..];
    assert_eq!(
        parse_git_manifest(test),
        Ok(vec![
            (&b"001e#"[..], &b"service=git-upload-pack"[..]),
            (&b"01234abc"[..], &b"heads"[..]),
            (&b"12345bcd"[..], &b"tags"[..]),
        ])
    );
    assert_eq!(test, &mut &b"0000"[..]);
}

#[test]
fn test_git_raw() {
    let mut options = HashMap::new();
    options.insert(
        "url".to_string(),
        "https://git.tuxfamily.org/bluebird/cms.git".to_string(),
    );
    let client = Client::new();
    let checker = GitChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
