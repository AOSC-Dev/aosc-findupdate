//! This module modifies the version string to comply with the [AOSC Package Styling Manual](https://wiki.aosc.io/developer/packaging/package-styling-manual/#versioning-variables).
use regex::{self, Regex};
/// Matches version strings with letter notation.
///
/// e.g. `1.2.3-p5`
///
/// The dash will be removed..
const REGEX_LETTER_NOTATION: &str = r"^\d+(?:\.\d+)+[-_~+^][a-z]\d+$";
/// Matches version strings with dashes
///
/// e.g. `2023-05-07`
///
/// We replace the dashes with dots.
const REGEX_DASHES: &str = r"^\d+(?:-\d+)+$";
/// Matches version strings with underscores
///
/// e.g. `10_2`
///
/// We replace the underscores with dots.
const REGEX_UNDERSCORES: &str = r"^\d+(?:_[0-9a-zA-Z]+)+$";
/// Matches version strings with release types (rc, beta, alpha)
///
/// e.g. `4.5-rc1`
///
/// We replace the symbol with a plus sign (`+`).
const REGEX_RELEASE_TYPES: &str = r"^\d+(?:\.\d+)+[-_~^]*(?:rc|a|alpha|b|beta)\d*$";
/// Matches version strings with revisions.
///
/// e.g. `5.4.3-2`
///
/// We replace the dash with tilde (`~`).
const REGEX_REVISION: &str = r"^\d+(?:\.\d+)+(?:-\d+)+$";
/// Trait for str
///
/// So one can modify the version string with version_str.comply_with_aosc().
pub trait VersionStr {
    fn compily_with_aosc(&self) -> String;
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum VersioningType {
    Normal,
    LetterNotation,
    Dashes,
    Underscores,
    ReleaseTypes,
    Revision,
}

fn version_type(version_string: &str) -> VersioningType {
    let matcher_letter_notation = Regex::new(REGEX_LETTER_NOTATION).unwrap();
    let matcher_dashes = Regex::new(REGEX_DASHES).unwrap();
    let matcher_underscores = Regex::new(REGEX_UNDERSCORES).unwrap();
    let matcher_release_types = Regex::new(REGEX_RELEASE_TYPES).unwrap();
    let matcher_revision = Regex::new(REGEX_REVISION).unwrap();
    if matcher_release_types.is_match(&version_string) {
        return VersioningType::ReleaseTypes
    }
    if matcher_dashes.is_match(&version_string) {
        return VersioningType::Dashes
    }
    if matcher_underscores.is_match(&version_string) {
        return VersioningType::Underscores
    }
    if matcher_letter_notation.is_match(&version_string) {
        return VersioningType::LetterNotation
    }
    if matcher_revision.is_match(version_string) {
        return VersioningType::Revision
    }
    VersioningType::Normal
}

impl VersionStr for str {
    /// Modifies the version string to comply with the [AOSC Package Styling Manual](https://wiki.aosc.io/developer/packaging/package-styling-manual/#versioning-variables).
    /// The searching regexes are strict enough to not to modify the part it is not supposed to do.
    fn compily_with_aosc(&self) -> String {
        let mut filtered_ver = self.to_lowercase();
        let versioning_type = version_type(&filtered_ver);
        match versioning_type {
            VersioningType::Normal => {
                // Nothing to do.
            }
            VersioningType::LetterNotation => {
                let replacer = Regex::new(r"[-_~+^]").unwrap();
                filtered_ver = replacer.replace_all(&filtered_ver.as_str(), "").to_string();
            }
            VersioningType::Dashes => {
                let replacer = Regex::new(r"[-_]").unwrap();
                filtered_ver = replacer.replace_all(&filtered_ver.as_str(), ".").to_string();
            }
            VersioningType::Underscores => {
                let replacer = Regex::new(r"[-_]").unwrap();
                filtered_ver = replacer.replace_all(&filtered_ver.as_str(), ".").to_string();
            }
            VersioningType::ReleaseTypes => {
                let replacer = Regex::new(r"[-+~^]*((?:rc|alpha|a|beta|b)\S+)").unwrap();
                filtered_ver = replacer.replace_all(&filtered_ver.as_str(), "~$1").to_string();
            }
            VersioningType::Revision => {
                let replacer = Regex::new(r"[-_~+^]").unwrap();
                filtered_ver = replacer.replace_all(&filtered_ver.to_string(), "+").to_string();
            }
        }
        filtered_ver
    }
}


#[test]
fn test_version_type() {
    let normal_version_str = &"1.2.3";
    let version_str_with_letter_notation = &"1.2.3-p6";
    let version_str_with_dashes = &"2023-07-18";
    let version_str_with_rev = &"6.4-20230718";
    let version_str_with_rel = &"5.3-56";
    let version_str_with_rc = &"0.9.1rc1";
    let version_str_with_rc_and_dash = &"2.16-rc1";
    let version_str_with_alpha = &"3.0-alpha5";
    let version_str_with_shortned_alpha = &"2.4a1";
    let version_str_with_shortned_beta = &"2.3b3";

    assert_eq!(version_type(normal_version_str), VersioningType::Normal);
    assert_eq!(version_type(version_str_with_letter_notation), VersioningType::LetterNotation);
    assert_eq!(version_type(version_str_with_dashes), VersioningType::Dashes);
    assert_eq!(version_type(version_str_with_rev), VersioningType::Revision);
    assert_eq!(version_type(version_str_with_rel), VersioningType::Revision);
    assert_eq!(version_type(version_str_with_alpha), VersioningType::ReleaseTypes);
    assert_eq!(version_type(version_str_with_rc), VersioningType::ReleaseTypes);
    assert_eq!(version_type(version_str_with_rc_and_dash), VersioningType::ReleaseTypes);
    assert_eq!(version_type(version_str_with_shortned_alpha), VersioningType::ReleaseTypes);
    assert_eq!(version_type(version_str_with_shortned_beta), VersioningType::ReleaseTypes);
}

#[test]
fn test_comply_with_aosc() {
    let normal_version_str = &"1.2.3";
    let version_str_with_letter_notation = &"1.2.3-p6";
    let version_str_with_dashes = &"2023-07-18";
    let version_str_with_rev = &"6.4-20230718";
    let version_str_with_rel = &"5.3-56";
    let version_str_with_rc = &"0.9.1rc1";
    let version_str_with_rc_and_dash = &"2.16-rc1";
    let version_str_with_alpha = &"3.0-alpha5";
    let version_str_with_shortned_alpha = &"2.4a1";
    let version_str_with_shortned_beta = &"2.3b3";
    assert_eq!(normal_version_str.compily_with_aosc(), String::from(normal_version_str.to_owned()));
    assert_eq!(version_str_with_letter_notation.compily_with_aosc(), String::from("1.2.3p6"));
    assert_eq!(version_str_with_dashes.compily_with_aosc(), String::from("2023.07.18"));
    assert_eq!(version_str_with_rev.compily_with_aosc(), String::from("6.4+20230718"));
    assert_eq!(version_str_with_rel.compily_with_aosc(), String::from("5.3+56"));
    assert_eq!(version_str_with_rc.compily_with_aosc(), String::from("0.9.1~rc1"));
    assert_eq!(version_str_with_rc_and_dash.compily_with_aosc(), String::from("2.16~rc1"));
    assert_eq!(version_str_with_alpha.compily_with_aosc(), String::from("3.0~alpha5"));
    assert_eq!(version_str_with_shortned_alpha.compily_with_aosc(), String::from("2.4~a1"));
    assert_eq!(version_str_with_shortned_beta.compily_with_aosc(), String::from("2.3~b3"));
}