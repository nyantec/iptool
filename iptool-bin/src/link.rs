use std::io::Result;

use clap::ArgMatches;
use iptool::IpTool;

use iptool::links::LinkTool;

pub fn link(sub_matches: &ArgMatches) -> Result<()> {
    match sub_matches.subcommand() {
        None => list_links(),
        _ => todo!(),
    }
}

fn list_links() -> Result<()> {
    let tool = LinkTool::new()?;

    let interfaces = tool.get_interfaces()?;

    for interface in interfaces {
        println!("{}", interface.print_info()?);
    }

    Ok(())
}
