use clap::{crate_version, App, Arg};

pub fn build_cli() -> App<'static, 'static> {
    App::new("aosc-findupdate")
        .version(crate_version!())
        .about("Find updated packages in the abbs tree")
        .arg(
            Arg::with_name("LOG")
                .short("l")
                .takes_value(true)
                .help("Log updated packages to a file"),
        )
        .arg(
            Arg::with_name("FILE")
                .short("f")
                .takes_value(true)
                .help("Path to a list of packages to be updated"),
        )
        .arg(
            Arg::with_name("INCLUDE")
                .short("i")
                .takes_value(true)
                .help("Use regular expression to filter which package to update"),
        )
        .arg(
            Arg::with_name("DIR")
                .short("d")
                .takes_value(true)
                .help("Specify the directory to the abbs tree"),
        )
}
