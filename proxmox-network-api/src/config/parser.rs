use crate::{PHYSICAL_NIC_REGEX, VLAN_INTERFACE_REGEX};

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::iter::{Iterator, Peekable};
use std::sync::LazyLock;

use anyhow::{bail, format_err, Error};
use regex::Regex;
use serde::de::{value, Deserialize, IntoDeserializer};

use super::helper::*;
use super::lexer::*;

use super::LinuxBondMode;

use proxmox_schema::api_types::IP_REGEX;

use super::{BondXmitHashPolicy, Interface, NetworkConfigMethod, NetworkInterfaceType};

use crate::config::NetworkConfig;
use crate::config::NetworkOrderEntry;

pub fn bond_mode_from_str(s: &str) -> Result<LinuxBondMode, Error> {
    LinuxBondMode::deserialize(s.into_deserializer())
        .map_err(|_: value::Error| format_err!("invalid bond_mode '{}'", s))
}

pub fn bond_xmit_hash_policy_from_str(s: &str) -> Result<BondXmitHashPolicy, Error> {
    BondXmitHashPolicy::deserialize(s.into_deserializer())
        .map_err(|_: value::Error| format_err!("invalid bond_xmit_hash_policy '{}'", s))
}

fn set_method_v4(iface: &mut Interface, method: NetworkConfigMethod) -> Result<(), Error> {
    if iface.method.is_none() {
        iface.method = Some(method);
    } else {
        bail!("inet configuration method already set.");
    }
    Ok(())
}

fn set_method_v6(iface: &mut Interface, method: NetworkConfigMethod) -> Result<(), Error> {
    if iface.method6.is_none() {
        iface.method6 = Some(method);
    } else {
        bail!("inet6 configuration method already set.");
    }
    Ok(())
}

fn set_cidr_v4(iface: &mut Interface, address: String) -> Result<(), Error> {
    if iface.cidr.is_none() {
        iface.cidr = Some(address);
    } else {
        bail!("duplicate IPv4 address.");
    }
    Ok(())
}

fn set_gateway_v4(iface: &mut Interface, gateway: String) -> Result<(), Error> {
    if iface.gateway.is_none() {
        iface.gateway = Some(gateway);
    } else {
        bail!("duplicate IPv4 gateway.");
    }
    Ok(())
}

fn set_cidr_v6(iface: &mut Interface, address: String) -> Result<(), Error> {
    if iface.cidr6.is_none() {
        iface.cidr6 = Some(address);
    } else {
        bail!("duplicate IPv6 address.");
    }
    Ok(())
}

fn set_gateway_v6(iface: &mut Interface, gateway: String) -> Result<(), Error> {
    if iface.gateway6.is_none() {
        iface.gateway6 = Some(gateway);
    } else {
        bail!("duplicate IPv4 gateway.");
    }
    Ok(())
}

fn set_interface_type(
    iface: &mut Interface,
    interface_type: NetworkInterfaceType,
) -> Result<(), Error> {
    if iface.interface_type == NetworkInterfaceType::Unknown {
        iface.interface_type = interface_type;
    } else if iface.interface_type != interface_type {
        bail!(
            "interface type already defined - cannot change from {:?} to {:?}",
            iface.interface_type,
            interface_type
        );
    }
    Ok(())
}

pub struct NetworkParser<R: BufRead> {
    input: Peekable<Lexer<R>>,
    line_nr: usize,
}

impl<R: BufRead> NetworkParser<R> {
    pub fn new(reader: R) -> Self {
        let input = Lexer::new(reader).peekable();
        Self { input, line_nr: 1 }
    }

    fn peek(&mut self) -> Result<Token, Error> {
        match self.input.peek() {
            Some(Err(err)) => {
                bail!("input error - {}", err);
            }
            Some(Ok((token, _))) => Ok(*token),
            None => {
                bail!("got unexpected end of stream (inside peek)");
            }
        }
    }

    fn next(&mut self) -> Result<(Token, String), Error> {
        match self.input.next() {
            Some(Err(err)) => {
                bail!("input error - {}", err);
            }
            Some(Ok((token, text))) => {
                if token == Token::Newline {
                    self.line_nr += 1;
                }
                Ok((token, text))
            }
            None => {
                bail!("got unexpected end of stream (inside peek)");
            }
        }
    }

    fn next_text(&mut self) -> Result<String, Error> {
        match self.next()? {
            (Token::Text, text) => Ok(text),
            (unexpected, _) => bail!("got unexpected token {:?} (expecting Text)", unexpected),
        }
    }

    fn eat(&mut self, expected: Token) -> Result<String, Error> {
        let (next, text) = self.next()?;
        if next != expected {
            bail!("expected {:?}, got {:?}", expected, next);
        }
        Ok(text)
    }

    fn parse_auto(&mut self, auto_flag: &mut HashSet<String>) -> Result<(), Error> {
        self.eat(Token::Auto)?;

        loop {
            match self.next()? {
                (Token::Text, iface) => {
                    auto_flag.insert(iface.to_string());
                }
                (Token::Newline, _) => break,
                unexpected => {
                    bail!("expected {:?}, got {:?}", Token::Text, unexpected);
                }
            }
        }

        Ok(())
    }

    fn parse_netmask(&mut self) -> Result<u8, Error> {
        self.eat(Token::Netmask)?;
        let netmask = self.next_text()?;

        let mask = if let Some(mask) = IPV4_MASK_HASH_LOCALNET.get(netmask.as_str()) {
            *mask
        } else {
            match netmask.as_str().parse::<u8>() {
                Ok(mask) => mask,
                Err(err) => {
                    bail!("unable to parse netmask '{}' - {}", netmask, err);
                }
            }
        };

        self.eat(Token::Newline)?;

        Ok(mask)
    }

    fn parse_iface_address(&mut self) -> Result<(String, Option<u8>, bool), Error> {
        self.eat(Token::Address)?;
        let cidr = self.next_text()?;

        let (_address, mask, ipv6) = parse_address_or_cidr(&cidr)?;

        self.eat(Token::Newline)?;

        Ok((cidr, mask, ipv6))
    }

    fn parse_iface_gateway(&mut self, interface: &mut Interface) -> Result<(), Error> {
        self.eat(Token::Gateway)?;
        let gateway = self.next_text()?;

        if IP_REGEX.is_match(&gateway) {
            if gateway.contains(':') {
                set_gateway_v6(interface, gateway)?;
            } else {
                set_gateway_v4(interface, gateway)?;
            }
        } else {
            bail!("unable to parse gateway address");
        }

        self.eat(Token::Newline)?;

        Ok(())
    }

    fn parse_iface_mtu(&mut self) -> Result<u64, Error> {
        self.eat(Token::MTU)?;

        let mtu = self.next_text()?;
        let mtu = match mtu.parse::<u64>() {
            Ok(mtu) => mtu,
            Err(err) => {
                bail!("unable to parse mtu value '{}' - {}", mtu, err);
            }
        };

        self.eat(Token::Newline)?;

        Ok(mtu)
    }

    fn parse_yes_no(&mut self) -> Result<bool, Error> {
        let text = self.next_text()?;
        let value = match text.to_lowercase().as_str() {
            "yes" => true,
            "no" => false,
            _ => {
                bail!("unable to bool value '{}' - (expected yes/no)", text);
            }
        };

        self.eat(Token::Newline)?;

        Ok(value)
    }

    fn parse_to_eol(&mut self) -> Result<String, Error> {
        let mut line = String::new();
        loop {
            match self.next()? {
                (Token::Newline, _) => return Ok(line),
                (_, text) => {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(&text);
                }
            }
        }
    }

    fn parse_iface_list(&mut self) -> Result<Vec<String>, Error> {
        let mut list = Vec::new();

        loop {
            let (token, text) = self.next()?;
            match token {
                Token::Newline => break,
                Token::Text => {
                    if &text != "none" {
                        list.push(text);
                    }
                }
                _ => bail!(
                    "unable to parse interface list - unexpected token '{:?}'",
                    token
                ),
            }
        }

        Ok(list)
    }

    fn parse_iface_attributes(
        &mut self,
        interface: &mut Interface,
        address_family_v4: bool,
        address_family_v6: bool,
    ) -> Result<(), Error> {
        let mut netmask = None;
        let mut address_list = Vec::new();

        loop {
            match self.peek()? {
                Token::Attribute => {
                    self.eat(Token::Attribute)?;
                }
                Token::Comment => {
                    let comment = self.eat(Token::Comment)?;
                    if !address_family_v4 && address_family_v6 {
                        let mut comments = interface.comments6.take().unwrap_or_default();
                        if !comments.is_empty() {
                            comments.push('\n');
                        }
                        comments.push_str(&comment);
                        interface.comments6 = Some(comments);
                    } else {
                        let mut comments = interface.comments.take().unwrap_or_default();
                        if !comments.is_empty() {
                            comments.push('\n');
                        }
                        comments.push_str(&comment);
                        interface.comments = Some(comments);
                    }
                    self.eat(Token::Newline)?;
                    continue;
                }
                _ => break,
            }

            match self.peek()? {
                Token::Address => {
                    let (cidr, mask, is_v6) = self.parse_iface_address()?;
                    address_list.push((cidr, mask, is_v6));
                }
                Token::Gateway => self.parse_iface_gateway(interface)?,
                Token::Netmask => {
                    //Note: netmask is deprecated, but we try to do our best
                    netmask = Some(self.parse_netmask()?);
                }
                Token::MTU => {
                    let mtu = self.parse_iface_mtu()?;
                    interface.mtu = Some(mtu);
                }
                Token::BridgeVlanAware => {
                    self.eat(Token::BridgeVlanAware)?;
                    let bridge_vlan_aware = self.parse_yes_no()?;
                    interface.bridge_vlan_aware = Some(bridge_vlan_aware);
                }
                Token::BridgePorts => {
                    self.eat(Token::BridgePorts)?;
                    let ports = self.parse_iface_list()?;
                    interface.bridge_ports = Some(ports);
                    set_interface_type(interface, NetworkInterfaceType::Bridge)?;
                }
                Token::BondSlaves => {
                    self.eat(Token::BondSlaves)?;
                    let slaves = self.parse_iface_list()?;
                    interface.slaves = Some(slaves);
                    set_interface_type(interface, NetworkInterfaceType::Bond)?;
                }
                Token::BondMode => {
                    self.eat(Token::BondMode)?;
                    let mode = self.next_text()?;
                    interface.bond_mode = Some(bond_mode_from_str(&mode)?);
                    self.eat(Token::Newline)?;
                }
                Token::BondPrimary => {
                    self.eat(Token::BondPrimary)?;
                    let primary = self.next_text()?;
                    interface.bond_primary = Some(primary);
                    self.eat(Token::Newline)?;
                }
                Token::BondXmitHashPolicy => {
                    self.eat(Token::BondXmitHashPolicy)?;
                    let policy = bond_xmit_hash_policy_from_str(&self.next_text()?)?;
                    interface.bond_xmit_hash_policy = Some(policy);
                    self.eat(Token::Newline)?;
                }
                Token::VlanId => {
                    self.eat(Token::VlanId)?;
                    let vlan_id = self.next_text()?.parse()?;
                    interface.vlan_id = Some(vlan_id);
                    set_interface_type(interface, NetworkInterfaceType::Vlan)?;
                    self.eat(Token::Newline)?;
                }
                Token::VlanRawDevice => {
                    self.eat(Token::VlanRawDevice)?;
                    let vlan_raw_device = self.next_text()?;
                    interface.vlan_raw_device = Some(vlan_raw_device);
                    set_interface_type(interface, NetworkInterfaceType::Vlan)?;
                    self.eat(Token::Newline)?;
                }
                _ => {
                    // parse addon attributes
                    let option = self.parse_to_eol()?;
                    if !option.is_empty() {
                        if !address_family_v4 && address_family_v6 {
                            interface.options6.push(option);
                        } else {
                            interface.options.push(option);
                        }
                    };
                }
            }
        }

        #[allow(clippy::comparison_chain)]
        if let Some(netmask) = netmask {
            if address_list.len() > 1 {
                bail!("unable to apply netmask to multiple addresses (please use cidr notation)");
            } else if address_list.len() == 1 {
                let (mut cidr, mask, is_v6) = address_list.pop().unwrap();
                if mask.is_some() {
                    // address already has a mask  - ignore netmask
                } else {
                    use std::fmt::Write as _;
                    check_netmask(netmask, is_v6)?;
                    let _ = write!(cidr, "/{}", netmask);
                }
                if is_v6 {
                    set_cidr_v6(interface, cidr)?;
                } else {
                    set_cidr_v4(interface, cidr)?;
                }
            } else {
                // no address - simply ignore useless netmask
            }
        } else {
            for (cidr, mask, is_v6) in address_list {
                if mask.is_none() {
                    bail!("missing netmask in '{}'", cidr);
                }
                if is_v6 {
                    set_cidr_v6(interface, cidr)?;
                } else {
                    set_cidr_v4(interface, cidr)?;
                }
            }
        }

        Ok(())
    }

    fn parse_iface(&mut self, config: &mut NetworkConfig) -> Result<(), Error> {
        self.eat(Token::Iface)?;
        let iface = self.next_text()?;

        let mut address_family_v4 = false;
        let mut address_family_v6 = false;
        let mut config_method = None;

        loop {
            let (token, text) = self.next()?;
            match token {
                Token::Newline => break,
                Token::Inet => address_family_v4 = true,
                Token::Inet6 => address_family_v6 = true,
                Token::Loopback => config_method = Some(NetworkConfigMethod::Loopback),
                Token::Static => config_method = Some(NetworkConfigMethod::Static),
                Token::Manual => config_method = Some(NetworkConfigMethod::Manual),
                Token::DHCP => config_method = Some(NetworkConfigMethod::DHCP),
                _ => bail!("unknown iface option {}", text),
            }
        }

        let config_method = config_method.unwrap_or(NetworkConfigMethod::Static);

        if !(address_family_v4 || address_family_v6) {
            address_family_v4 = true;
            address_family_v6 = true;
        }

        if let Some(interface) = config.interfaces.get_mut(&iface) {
            if address_family_v4 {
                set_method_v4(interface, config_method)?;
            }
            if address_family_v6 {
                set_method_v6(interface, config_method)?;
            }

            self.parse_iface_attributes(interface, address_family_v4, address_family_v6)?;
        } else {
            let mut interface = Interface::new(iface.clone());
            if address_family_v4 {
                set_method_v4(&mut interface, config_method)?;
            }
            if address_family_v6 {
                set_method_v6(&mut interface, config_method)?;
            }

            self.parse_iface_attributes(&mut interface, address_family_v4, address_family_v6)?;

            config.interfaces.insert(interface.name.clone(), interface);

            config.order.push(NetworkOrderEntry::Iface(iface));
        }

        Ok(())
    }

    pub fn parse_interfaces(
        &mut self,
        existing_interfaces: Option<&HashMap<String, IpLink>>,
    ) -> Result<NetworkConfig, Error> {
        self.do_parse_interfaces(existing_interfaces)
            .map_err(|err| format_err!("line {}: {}", self.line_nr, err))
    }

    fn do_parse_interfaces(
        &mut self,
        existing_interfaces: Option<&HashMap<String, IpLink>>,
    ) -> Result<NetworkConfig, Error> {
        let mut config = NetworkConfig::new();

        let mut auto_flag: HashSet<String> = HashSet::new();

        loop {
            match self.peek()? {
                Token::EOF => {
                    break;
                }
                Token::Newline => {
                    // skip empty lines
                    self.eat(Token::Newline)?;
                }
                Token::Comment => {
                    let (_, text) = self.next()?;
                    config.order.push(NetworkOrderEntry::Comment(text));
                    self.eat(Token::Newline)?;
                }
                Token::Auto => {
                    self.parse_auto(&mut auto_flag)?;
                }
                Token::Iface => {
                    self.parse_iface(&mut config)?;
                }
                _ => {
                    let option = self.parse_to_eol()?;
                    if !option.is_empty() {
                        config.order.push(NetworkOrderEntry::Option(option));
                    }
                }
            }
        }

        for iface in auto_flag.iter() {
            if let Some(interface) = config.interfaces.get_mut(iface) {
                interface.autostart = true;
            }
        }

        static INTERFACE_ALIAS_REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^\S+:\d+$").unwrap());

        if let Some(existing_interfaces) = existing_interfaces {
            for (iface, ip_link) in existing_interfaces.iter() {
                if let Some(interface) = config.interfaces.get_mut(iface) {
                    interface.active = ip_link.active();
                    if interface.interface_type == NetworkInterfaceType::Unknown
                        && ip_link.is_physical()
                    {
                        interface.interface_type = NetworkInterfaceType::Eth;
                    }

                    interface.altnames = ip_link.altnames().cloned().collect();
                } else if ip_link.is_physical() {
                    // also add all physical NICs
                    let mut interface = Interface::new(iface.clone());
                    set_method_v4(&mut interface, NetworkConfigMethod::Manual)?;
                    interface.interface_type = NetworkInterfaceType::Eth;
                    interface.active = ip_link.active();
                    config.interfaces.insert(interface.name.clone(), interface);
                    config
                        .order
                        .push(NetworkOrderEntry::Iface(iface.to_string()));
                }
            }
        }

        for (name, interface) in config.interfaces.iter_mut() {
            if interface.interface_type != NetworkInterfaceType::Unknown {
                continue;
            }
            if name == "lo" {
                interface.interface_type = NetworkInterfaceType::Loopback;
                continue;
            }
            if INTERFACE_ALIAS_REGEX.is_match(name) {
                interface.interface_type = NetworkInterfaceType::Alias;
                continue;
            }
            if VLAN_INTERFACE_REGEX.is_match(name) {
                interface.interface_type = NetworkInterfaceType::Vlan;
                continue;
            }

            match existing_interfaces {
                Some(existing_interfaces) => {
                    let is_physical = existing_interfaces
                        .get(name)
                        .map(|iface| iface.is_physical())
                        .unwrap_or(false);

                    if is_physical {
                        interface.interface_type = NetworkInterfaceType::Eth;
                        continue;
                    }
                }
                None if PHYSICAL_NIC_REGEX.is_match(name) => {
                    interface.interface_type = NetworkInterfaceType::Eth;
                    continue;
                }
                _ => {}
            }
        }

        if !config.interfaces.contains_key("lo") {
            let mut interface = Interface::new(String::from("lo"));
            set_method_v4(&mut interface, NetworkConfigMethod::Loopback)?;
            interface.interface_type = NetworkInterfaceType::Loopback;
            interface.autostart = true;
            config.interfaces.insert(interface.name.clone(), interface);

            // Note: insert 'lo' as first interface after initial comments
            let mut new_order = Vec::new();
            let mut added_lo = false;
            for entry in config.order {
                if added_lo {
                    new_order.push(entry);
                    continue;
                } // copy the rest
                match entry {
                    NetworkOrderEntry::Comment(_) => {
                        new_order.push(entry);
                    }
                    _ => {
                        new_order.push(NetworkOrderEntry::Iface(String::from("lo")));
                        added_lo = true;
                        new_order.push(entry);
                    }
                }
            }
            config.order = new_order;
        }

        Ok(config)
    }
}

#[cfg(test)]
mod test {

    use anyhow::Error;

    use super::*;

    #[test]
    fn test_network_config_create_lo_1() -> Result<(), Error> {
        let input = "";

        let mut parser = NetworkParser::new(input.as_bytes());

        let config = parser.parse_interfaces(None)?;

        let output = String::try_from(config)?;

        let expected = "auto lo\niface lo inet loopback\n\n";
        assert_eq!(output, expected);

        // run again using output as input
        let mut parser = NetworkParser::new(output.as_bytes());

        let config = parser.parse_interfaces(None)?;

        let output = String::try_from(config)?;

        assert_eq!(output, expected);

        Ok(())
    }

    #[test]
    fn test_network_config_create_lo_2() -> Result<(), Error> {
        let input = "#c1\n\n#c2\n\niface test inet manual\n";

        let mut parser = NetworkParser::new(input.as_bytes());

        let config = parser.parse_interfaces(None)?;

        let output = String::try_from(config)?;

        // Note: loopback should be added in front of other interfaces
        let expected = "#c1\n#c2\n\nauto lo\niface lo inet loopback\n\niface test inet manual\n\n";
        assert_eq!(output, expected);

        Ok(())
    }

    #[test]
    fn test_network_config_parser_no_blank_1() -> Result<(), Error> {
        let input = "auto lo\n\
                     iface lo inet loopback\n\
                     iface lo inet6 loopback\n\
                     auto ens18\n\
                     iface ens18 inet static\n\
                     \taddress 192.168.20.144/20\n\
                     \tgateway 192.168.16.1\n\
                     # comment\n\
                     iface ens20 inet static\n\
                     \taddress 192.168.20.145/20\n\
                     iface ens21 inet manual\n\
                     iface ens22 inet manual\n";

        let mut parser = NetworkParser::new(input.as_bytes());

        let config = parser.parse_interfaces(None)?;

        let output = String::try_from(config)?;

        let expected = "auto lo\n\
                        iface lo inet loopback\n\
                        \n\
                        iface lo inet6 loopback\n\
                        \n\
                        auto ens18\n\
                        iface ens18 inet static\n\
                        \taddress 192.168.20.144/20\n\
                        \tgateway 192.168.16.1\n\
                        #comment\n\
                        \n\
                        iface ens20 inet static\n\
                        \taddress 192.168.20.145/20\n\
                        \n\
                        iface ens21 inet manual\n\
                        \n\
                        iface ens22 inet manual\n\
                        \n";
        assert_eq!(output, expected);

        Ok(())
    }

    #[test]
    fn test_network_config_parser_no_blank_2() -> Result<(), Error> {
        // Adapted from bug 2926
        let input = "### Hetzner Online GmbH installimage\n\
                     \n\
                     source /etc/network/interfaces.d/*\n\
                     \n\
                     auto lo\n\
                     iface lo inet loopback\n\
                     iface lo inet6 loopback\n\
                     \n\
                     auto enp4s0\n\
                     iface enp4s0 inet static\n\
                     \taddress 10.10.10.10/24\n\
                     \tgateway 10.10.10.1\n\
                     \t# route 10.10.20.10/24 via 10.10.20.1\n\
                     \tup route add -net 10.10.20.10 netmask 255.255.255.0 gw 10.10.20.1 dev enp4s0\n\
                     \n\
                     iface enp4s0 inet6 static\n\
                     \taddress fe80::5496:35ff:fe99:5a6a/64\n\
                     \tgateway fe80::1\n";

        let mut parser = NetworkParser::new(input.as_bytes());

        let config = parser.parse_interfaces(None)?;

        let output = String::try_from(config)?;

        let expected = "### Hetzner Online GmbH installimage\n\
                        \n\
                        source /etc/network/interfaces.d/*\n\
                        \n\
                        auto lo\n\
                        iface lo inet loopback\n\
                        \n\
                        iface lo inet6 loopback\n\
                        \n\
                        auto enp4s0\n\
                        iface enp4s0 inet static\n\
                        \taddress 10.10.10.10/24\n\
                        \tgateway 10.10.10.1\n\
                        \t# route 10.10.20.10/24 via 10.10.20.1\n\
                        \tup route add -net 10.10.20.10 netmask 255.255.255.0 gw 10.10.20.1 dev enp4s0\n\
                        \n\
                        iface enp4s0 inet6 static\n\
                        \taddress fe80::5496:35ff:fe99:5a6a/64\n\
                        \tgateway fe80::1\n\
                        \n";
        assert_eq!(output, expected);

        Ok(())
    }

    #[test]
    fn test_network_config_parser_vlan_id_in_name() {
        let input = "iface vmbr0.100 inet static manual";
        let mut parser = NetworkParser::new(input.as_bytes());
        let config = parser.parse_interfaces(None).unwrap();

        let iface = config.interfaces.get("vmbr0.100").unwrap();
        assert_eq!(iface.interface_type, NetworkInterfaceType::Vlan);
        assert_eq!(iface.vlan_raw_device, None);
        assert_eq!(iface.vlan_id, None);
    }

    #[test]
    fn test_network_config_parser_vlan_with_raw_device() {
        let input = r#"
iface vlan100 inet manual
	vlan-raw-device vmbr0"#;

        let mut parser = NetworkParser::new(input.as_bytes());
        let config = parser.parse_interfaces(None).unwrap();

        let iface = config.interfaces.get("vlan100").unwrap();
        assert_eq!(iface.interface_type, NetworkInterfaceType::Vlan);
        assert_eq!(iface.vlan_raw_device, Some(String::from("vmbr0")));
        assert_eq!(iface.vlan_id, None);
    }

    #[test]
    fn test_network_config_parser_vlan_with_raw_device_static() {
        let input = r#"
iface vlan100 inet static
	vlan-raw-device vmbr0
	address 10.0.0.100/16"#;

        let mut parser = NetworkParser::new(input.as_bytes());
        let config = parser.parse_interfaces(None).unwrap();

        let iface = config.interfaces.get("vlan100").unwrap();
        assert_eq!(iface.interface_type, NetworkInterfaceType::Vlan);
        assert_eq!(iface.vlan_raw_device, Some(String::from("vmbr0")));
        assert_eq!(iface.vlan_id, None);
        assert_eq!(iface.method, Some(NetworkConfigMethod::Static));
        assert_eq!(iface.cidr, Some(String::from("10.0.0.100/16")));
    }

    #[test]
    fn test_network_config_parser_vlan_individual_name() {
        let input = r#"
iface individual_name inet manual
	vlan-id 100
	vlan-raw-device vmbr0"#;

        let mut parser = NetworkParser::new(input.as_bytes());
        let config = parser.parse_interfaces(None).unwrap();

        let iface = config.interfaces.get("individual_name").unwrap();
        assert_eq!(iface.interface_type, NetworkInterfaceType::Vlan);
        assert_eq!(iface.vlan_raw_device, Some(String::from("vmbr0")));
        assert_eq!(iface.vlan_id, Some(100));
    }

    #[test]
    fn test_network_config_parser_vlan_individual_name_static() {
        let input = r#"
iface individual_name inet static
	vlan-id 100
	vlan-raw-device vmbr0
	address 10.0.0.100/16
"#;

        let mut parser = NetworkParser::new(input.as_bytes());
        let config = parser.parse_interfaces(None).unwrap();

        let iface = config.interfaces.get("individual_name").unwrap();
        assert_eq!(iface.interface_type, NetworkInterfaceType::Vlan);
        assert_eq!(iface.vlan_raw_device, Some(String::from("vmbr0")));
        assert_eq!(iface.vlan_id, Some(100));
        assert_eq!(iface.method, Some(NetworkConfigMethod::Static));
        assert_eq!(iface.cidr, Some(String::from("10.0.0.100/16")));
    }
}
