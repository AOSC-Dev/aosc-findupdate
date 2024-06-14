use std::collections::HashMap;

use super::{extract_versions, version_compare, UpdateChecker};
use crate::must_have;
use anyhow::{anyhow, bail, Result};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use winnow::{
    ascii::{multispace1, space1, till_line_ending},
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
    separated_pair(first_tuple, space1, till_line_ending).parse_next(input)
}

fn single_line<'a>(input: &mut &'a [u8]) -> PResult<(&'a [u8], &'a [u8])> {
    terminated(kv_pair, multispace1).parse_next(input)
}

fn parse_git_manifest<'a>(input: &mut &'a [u8]) -> PResult<Vec<(&'a [u8], &'a [u8])>> {
    repeat(1.., single_line).parse_next(input)
}

pub enum GitRefs<'a> {
    Tag(&'a str),
    Heads(&'a str, &'a str),
}

impl ToString for GitRefs<'_> {
    fn to_string(&self) -> String {
        match self {
            GitRefs::Tag(name) => name.to_string(),
            GitRefs::Heads(name, _) => name.to_string(),
        }
    }
}
// end of parser-combinators
fn collect_git_refs<'a>(input: &mut &'a [u8]) -> Result<Vec<GitRefs<'a>>> {
    let tuples = parse_git_manifest(input).map_err(|e| anyhow!("Parser error: {:?}", e))?;
    let tags: Vec<_> = tuples
        .iter()
        .filter_map(|x| {
            if x.1.ends_with(&b"^{}"[..]) {
                None
            } else if let Some(name) = x.1.strip_prefix(&b"refs/tags/"[..]) {
                if let Ok(name) = std::str::from_utf8(name) {
                    Some(GitRefs::Tag(name))
                } else {
                    None
                }
            } else if let Some(head_name) = x.1.strip_prefix(&b"refs/heads/"[..]) {
                if let (Ok(head_name), Ok(rev)) =
                    (std::str::from_utf8(head_name), std::str::from_utf8(x.0))
                {
                    Some(GitRefs::Heads(head_name, rev))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    Ok(tags)
}

pub(crate) struct GitChecker {
    url: String,
    branch: Option<String>,
    pattern: Option<String>,
}

impl UpdateChecker for GitChecker {
    fn new(config: &HashMap<String, String>) -> Result<Self>
    where
        Self: Sized + UpdateChecker,
    {
        let url = must_have!(config, "url", "Repository URL")?.to_string();
        let pattern = config.get("pattern").cloned();
        let branch = config.get("branch").cloned();

        Ok(GitChecker {
            url,
            pattern,
            branch,
        })
    }

    fn check(&self, client: &Client) -> Result<String> {
        // this check method uses a fake Git client implementation
        let resp = client
            .get(format!("{}/info/refs?service=git-upload-pack", self.url,))
            .header(USER_AGENT, format!("git/{}", SIMULATED_GIT_VERSION))
            .header("git-protocol", "version=2")
            .send()?;
        resp.error_for_status_ref()?;
        let body = resp.bytes()?;
        let body = body.to_vec();
        let mut body = body.as_ref();

        let mut head: Box<dyn Iterator<Item = _>> =
            Box::new(collect_git_refs(&mut body)?.into_iter());

        if let Some(branch) = &self.branch {
            head = Box::new(head.filter(move |x| match x {
                GitRefs::Heads(head_name, _) => head_name == branch,
                _ => false,
            }));

            let head = head.next().map(|x| {
                if let GitRefs::Heads(_, rev) = x {
                    rev
                } else {
                    unreachable!()
                }
            });

            match head {
                Some(head) => Ok(head.to_string()),
                None => bail!("Git ({}) branch didn't return any rev!", self.url),
            }
        } else {
            head = Box::new(head.filter(|x| match x {
                GitRefs::Tag(_) => true,
                _ => false,
            }));

            let mut head = head.map(|x| x.to_string()).collect::<Vec<_>>();

            if let Some(pattern) = &self.pattern {
                head = extract_versions(pattern, &head)?;
            }

            if head.is_empty() {
                return Err(anyhow!("Git ({}) didn't return any tags!", self.url));
            }

            head.sort_unstable_by(|b, a| version_compare(a, b));

            Ok(head.first().unwrap().to_string())
        }
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

#[test]
fn test_git_branch_raw() {
    let mut options = HashMap::new();
    options.insert(
        "url".to_string(),
        "https://git.tuxfamily.org/bluebird/cms.git".to_string(),
    );
    options.insert("branch".to_string(), "master".to_string());
    let client = Client::new();
    let checker = GitChecker::new(&options).unwrap();
    dbg!(checker.check(&client).unwrap());
}
