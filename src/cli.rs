use clap::{Arg, Command};

pub fn build_cli() -> Command {
    Command::new("aosc-findupdate")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Find updated packages in the abbs tree")
        .arg(
            Arg::new("DRY_RUN")
                .long("dry-run")
                .help("Do not update the files in the abbs tree"),
        )
        .arg(
            Arg::new("LOG")
                .short('l')
                .num_args(1)
                .help("Log updated packages to a file"),
        )
        .arg(
            Arg::new("FILE")
                .short('f')
                .num_args(1)
                .help("Path to a list of packages to be updated"),
        )
        .arg(
            Arg::new("INCLUDE")
                .short('i')
                .num_args(1)
                .help("Use regular expression to filter which package to update"),
        )
        .arg(
            Arg::new("DIR")
                .short('d')
                .num_args(1)
                .help("Specify the directory to the abbs tree"),
        )
}
