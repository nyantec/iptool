use anyhow::{anyhow, bail, Result};
use clap::{App, Arg, ArgMatches};
use iptool::links::LinkTool;
use iptool::IpTool;

fn dev() -> Arg<'static> {
    Arg::new("dev").long("dev").takes_value(true)
}

pub(crate) fn app() -> App<'static> {
    App::new("link")
        .about("Link management")
        .alias("l")
        .subcommand(
            App::new("show")
                .alias("s")
                .about("Show links")
                .arg(dev())
                .arg(Arg::new("grep").long("grep").short('g').takes_value(true))
                .arg(
                    Arg::new("netns")
                        .long("netns")
                        .alias("ns")
                        .takes_value(true),
                ),
        )
        .subcommand(
            App::new("set")
                .about("Manage link")
                .arg(dev().required(true))
                .arg(Arg::new("state"))
                .arg(Arg::new("mtu").long("mtu").takes_value(true))
                .arg(
                    Arg::new("netns")
                        .long("netns")
                        .alias("ns")
                        .takes_value(true),
                ),
        )
        .subcommand(
            App::new("delete")
                .alias("d")
                .about("Delete links")
                .arg(dev().required(true)),
        )
}

pub fn link(sub_matches: &ArgMatches) -> Result<()> {
    match sub_matches.subcommand() {
        None => list_links(sub_matches, false),
        Some(("show", matches)) => list_links(matches, true),
        Some(("set", matches)) => set_link(matches),
        Some(("delete", matches)) => del_link(matches),
        _ => todo!(),
    }
}

fn list_links(sub_matches: &ArgMatches, has_name: bool) -> Result<()> {
    let dev = if has_name {
        sub_matches.value_of("dev")
    } else {
        None
    };
    let grep = if has_name {
        sub_matches.value_of("grep")
    } else {
        None
    };
    let tool = LinkTool::new()?;

    let nsid =
        if has_name {
            if let Some(name) = sub_matches.value_of("netns") {
                Some(crate::netns::get_named_nsid(&tool, name)?.ok_or_else(|| {
                    anyhow!("No registered network namespace found for '{}'", name)
                })?)
            } else {
                None
            }
        } else {
            None
        };

    if let Some(dev) = dev {
        //let interface = tool.get_interface(dev)?;
        let interface = unsafe { tool.get_inner() }.get_interface_ns(dev, nsid)?;
        println!("{}", interface.print_info()?);
    } else {
        let interfaces = unsafe { tool.get_inner() }.get_interfaces_ns(nsid)?;
        for interface in interfaces {
            if let Some(grep) = grep {
                if !interface.get_name().contains(grep) {
                    continue;
                }
            }
            println!("{}", interface.print_info()?);
        }
    }

    Ok(())
}

fn set_link(sub_matches: &ArgMatches) -> Result<()> {
    let mut tool = IpTool::new()?;
    let mut link = LinkTool::new()?;

    let dev = sub_matches.value_of("dev").unwrap();

    // netns
    sub_matches
        .value_of("netns")
        .map(|ns| set_link_netns(&mut link, dev, ns))
        .transpose()?;

    // state
    sub_matches
        .value_of("state")
        .map(|state| set_link_state(&mut tool, dev, state))
        .transpose()?;

    // mtu
    sub_matches
        .value_of("mtu")
        .map(|x| x.parse::<u32>().ok())
        .flatten()
        .map(|x| tool.set_mut(dev, x))
        .transpose()?;

    Ok(())
}

fn set_link_state(tool: &mut IpTool, dev: &str, state: &str) -> Result<()> {
    let state = match state.to_lowercase().as_str() {
        "up" => true,
        "down" => false,
        _ => bail!("Invalid state verb"),
    };

    tool.set_up(dev, state)?;

    Ok(())
}

fn set_link_netns(tool: &mut LinkTool, dev: &str, ns: &str) -> Result<()> {
    // check if ns is a valid nsid and use directly without fd
    let mut path = std::path::PathBuf::from(crate::netns::IP_NETNS_PATH);
    path.push(ns);

    unsafe { tool.get_inner_mut() }.set_interface_ns_path(dev, &path)?;

    Ok(())
}

fn del_link(sub_matches: &ArgMatches) -> Result<()> {
    let dev = sub_matches.value_of("dev").unwrap();

    let mut tool = LinkTool::new()?;

    tool.delete_interface(tool.get_interface(dev)?)?;

    Ok(())
}
