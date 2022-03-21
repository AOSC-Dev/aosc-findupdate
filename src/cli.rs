use clap::{Arg, Command};

pub fn build_cli() -> Command<'static> {
    Command::new("aosc-findupdate")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Find updated packages in the abbs tree")
        .arg(
            Arg::new("DRY_RUN")
                .long("dry-run")
                .takes_value(false)
                .help("Do not update the files in the abbs tree"),
        )
        .arg(
            Arg::new("LOG")
                .short('l')
                .takes_value(true)
                .help("Log updated packages to a file"),
        )
        .arg(
            Arg::new("FILE")
                .short('f')
                .takes_value(true)
                .help("Path to a list of packages to be updated"),
        )
        .arg(
            Arg::new("INCLUDE")
                .short('i')
                .takes_value(true)
                .help("Use regular expression to filter which package to update"),
        )
        .arg(
            Arg::new("DIR")
                .short('d')
                .takes_value(true)
                .help("Specify the directory to the abbs tree"),
        )
}
