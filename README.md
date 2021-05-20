# AOSC Find Update

This is a small utility that helps AOSC developers to find package updates in the abbs tree.

## Requirements

- Rust 1.51+
- OpenSSL

## Building

`cargo build` or `cargo build --release`

## Usage

```
USAGE:
    aosc-findupdate [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -d <DIR>            Specify the directory to the abbs tree
    -f <FILE>           Path to a list of packages to be updated
    -i <INCLUDE>        Use regular expression to filter which package to update
```

### Suggested Usages

- Scenario: Rebuild + Update Survey

Example: Do a Rust rebuild survey: `aosc-findupdate -f groups/rust-rebuilds`

- Scenario: General Survey

Example: Do a general survey with all packages matching the pattern "extra-d*": `aosc-findupdate -i 'extra-d.+'`

(Note that the pattern is in **Regex syntax**, not bash globbing syntax!)
