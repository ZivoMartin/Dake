use std::{net::IpAddr, path::PathBuf, str::FromStr};

use anyhow::{Error, Result, bail};

pub const DIRECTIVE_PREFIX: &str = "#!";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Directive {
    RootDef { ip: IpAddr, path: PathBuf },
}

impl FromStr for Directive {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let words = s
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>();
        Ok(match &words[..] {
            ["ROOT_DEF", ip, "=", path] => Directive::RootDef {
                ip: ip.parse()?,
                path: path.parse()?,
            },
            _ => bail!("Invalid Dake directive: {}", s),
        })
    }
}
