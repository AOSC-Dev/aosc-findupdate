# AOSC Find Update

This is a small utility that helps AOSC developers to find package updates in the abbs tree.

## Requirements

- Rust 1.51+
- OpenSSL

## Building

`cargo build` or `cargo build --release`

## Usage

```
USAGE: aosc-findupdate [OPTIONS]

OPTIONS:
      --dry-run     Do not update the files in the abbs tree
  -l <LOG>          Log updated packages to a file
  -f <FILE>         Path to a list of packages to be updated
  -i <INCLUDE>      Use regular expression to filter which package to update
  -d <DIR>          Specify the directory to the abbs tree
  -c                Modify version strings to comply with the AOSC Package Styling Manual
  -h, --help        Print help
  -V, --version     Print version
```

### Suggested Usages

- Scenario: Rebuild + Update Survey

Example: Do a Rust rebuild survey: `aosc-findupdate -f groups/rust-rebuilds`

- Scenario: General Survey

Example: Do a general survey with all packages matching the pattern "extra-d*": `aosc-findupdate -i 'extra-d.+'`

(Note that the pattern is in **Regex syntax**, not bash globbing syntax!)


### AOSC OS Package Styling Manual compliance

AOSC Find Update does not comply with the [AOSC OS Package Styling Manual](https://wiki.aosc.io/developer/packaging/package-styling-manual/#versioning-variables) by default, unless the `-c` switch is enabled.

With the `-c` switch enabled, AOSC Find Update will transform the returned version number automatically in accordance with the Styling Manual. However, the regexes to transform version numbers are not strict enough to prevent unwanted modifications.

Always double check your `spec` file if you have enabled the `-c` switch.