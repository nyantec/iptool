mod link;

use clap::{App, AppSettings, ArgMatches};
use std::io::Result;

fn build_app() -> App<'static> {
    App::new("iprs")
        .about("A iprout2 implementation in rust")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        //.setting(AppSettings::AllowExternalSubcommands) // TODO: yeah?
        //.setting(AppSettings::AllowInvalidUtf8ForExternalSubcommands)
        .subcommand(App::new("link").about("Link managemant"))
}

fn main() -> Result<()> {
    let app = build_app();

    let matches = app.get_matches();

    match matches.subcommand() {
        Some(("link", sub_matches)) => link::link(sub_matches),
        _ => unreachable!(),
    }
}
