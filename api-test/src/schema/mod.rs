//! Common schema definitions.

use proxmox::api::api;

pub mod memory;
pub mod string_list;
pub mod string_set;
pub mod tools;
pub mod types;

#[api({
    cli: false,
    description:
        r"Startup and shutdown behavior. \
          Order is a non-negative number defining the general startup order. \
          Shutdown in done with reverse ordering. \
          Additionally you can set the 'up' or 'down' delay in seconds, which specifies a delay \
          to wait before the next VM is started or stopped.",
    fields: {
        order: "Absolute ordering",
        up: "Delay to wait before moving on to the next VM during startup.",
        down: "Delay to wait before moving on to the next VM during shutdown.",
    },
})]
#[derive(Default)]
pub struct StartupOrder {
    pub order: Option<usize>,
    pub up: Option<usize>,
    pub down: Option<usize>,
}

#[api({description: "Architecture."})]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    // FIXME: suppport: #[api(alternatives = ["x86_64"])]
    Amd64,
    I386,
    Arm64,
    Armhf,
}

pub mod dns_name {
    use lazy_static::lazy_static;
    use regex::Regex;

    pub const NAME: &str = "DNS name";

    lazy_static! {
        //static ref DNS_BASE_RE: Regex =
        //    Regex::new(r#"(?:[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?)"#).unwrap();
        static ref REGEX: Regex =
            Regex::new(r#"^(?x)
                (?:[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?)
                (?:\.(?:[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?))*
                $"#).unwrap();
    }

    pub fn verify<T: crate::schema::tools::StringContainer>(value: &T) -> bool {
        value.all(|s| REGEX.is_match(s))
    }
}

pub mod ip_address {
    pub const NAME: &str = "IP Address";

    pub fn verify<T: crate::schema::tools::StringContainer>(value: &T) -> bool {
        value.all(|s| proxmox::tools::common_regex::IP_REGEX.is_match(s))
    }
}

pub mod safe_path {
    pub const NAME: &str = "A canonical, absolute file system path";

    pub fn verify<T: crate::schema::tools::StringContainer>(value: &T) -> bool {
        value.all(|s| {
            s != ".." && !s.starts_with("../") && !s.ends_with("/..") && !s.contains("/../")
        })
    }
}
