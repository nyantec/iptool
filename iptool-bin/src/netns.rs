use anyhow::Result;
use clap::{App, Arg, ArgMatches};
use iptool::links::LinkTool;
use std::path::PathBuf;

// TODO: move to netns module
pub const IP_NETNS_PATH: &'static str = "/var/run/netns";

pub fn app() -> App<'static> {
    App::new("netns")
        .about("Network Namespace management")
        .subcommand(App::new("exec").arg(Arg::new("netns").takes_value(true).required(true)))
}

pub fn netns(sub_matches: &ArgMatches) -> Result<()> {
    match sub_matches.subcommand() {
        None => todo!("List known namespaces"),
        Some(("exec", matches)) => exec(matches),
        _ => todo!(),
    }
}

// TODO: detect if name/absolute path or id and enter respectively
fn exec(sub_matches: &ArgMatches) -> Result<()> {
    let netns = sub_matches.value_of("netns").unwrap();

    let mut tool = LinkTool::new()?;

    println!("entering ns: {}", netns);
    let mut path = PathBuf::from(IP_NETNS_PATH);
    path.push(netns);
    // SAFETY: execving own process, nothing depends on the current `unshare` state
    unsafe { tool.enter_ns_path(&path) }?;

    //nix::unistd::execv(CString::try_from("/bin/sh".to_owned())?.as_c_str(), &[]);
    let bash = std::ffi::CString::new("/bin/sh")?;
    nix::unistd::execv::<&std::ffi::CStr>(bash.as_c_str(), &[])?;
    Ok(())
}

// TODO: check if name is an absolute path or an nsid number, and use it
pub fn get_named_nsid(tool: &LinkTool, name: &str) -> Result<Option<i32>> {
    let mut path = PathBuf::from(IP_NETNS_PATH);
    path.push(name);

    Ok(unsafe { tool.get_inner() }.get_nsid_path(&path)?)
}
