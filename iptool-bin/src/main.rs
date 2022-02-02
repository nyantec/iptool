mod link;

use anyhow::Result;
use clap::{App, AppSettings, ColorChoice, Values};
use std::collections::HashMap;

fn build_app() -> App<'static> {
    App::new("iprs")
        .about("A iprout2 implementation in rust")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .color(ColorChoice::Auto)
        //.setting(AppSettings::AllowExternalSubcommands) // TODO: yeah?
        //.setting(AppSettings::AllowInvalidUtf8ForExternalSubcommands)
        .subcommand(link::app())
}

fn main() -> Result<()> {
    let app = build_app();

    let matches = app.get_matches();

    match matches.subcommand() {
        Some(("link", sub_matches)) => link::link(sub_matches),
        _ => unreachable!(),
    }
}

/*pub(crate) fn parse_query<'a>(values: Values<'a>) -> Result<HashMap<&'a str, &'a str>> {
    let mut ret = HashMap::new();

    //let mut iter = values.peekable();
    let mut iter = values;

    for key in iter.next() {
        if let Some(value) = iter.next() {
            ret.insert(key, value);
        } else {
            return Err(ErrorKind::InvalidData.into());
        }
    }

    Ok(ret)
}*/

/*pub(crate) fn to_vec<'a>(mut values: Values<'a>) -> Vec<&'a str> {
    /*let mut ret = Vec::new();

    for x in values {
        ret.push(x);
    }

    ret*/
    values.collect()
}

pub(crate) fn find_arg<'a>(values: &Vec<&'a str>, key: &str) -> Result<Option<&'a str>> {
    for i in 0..values.len() {
        let ckey = values.get(i).unwrap();
        if *ckey == key {
            return values
                .get(i + 1)
                .map(|x| Some(*x))
                .ok_or(ErrorKind::InvalidData.into());
        }
    }

    Ok(None)
}*/
