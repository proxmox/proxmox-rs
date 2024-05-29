use anyhow::{bail, Error};

use proxmox_config_digest::ConfigDigest;

use crate::network::{self, parse_vlan_id_from_name, parse_vlan_raw_device_from_name};
use crate::network::{
    DeletableInterfaceProperty, Interface, InterfaceUpdater, LinuxBondMode, NetworkConfigMethod,
    NetworkInterfaceType,
};

/// Create network interface configuration.
pub fn create_interface(iface: String, config: InterfaceUpdater) -> Result<(), Error> {
    let interface_type = match config.interface_type {
        Some(interface_type) => interface_type,
        None => bail!("missing parameter 'type'"),
    };

    let _lock = network::lock_config()?;

    let (mut network_config, _digest) = network::config()?;

    if network_config.interfaces.contains_key(&iface) {
        bail!("interface '{}' already exists", iface);
    }

    let mut interface = Interface::new(iface.clone());
    interface.interface_type = interface_type;

    if let Some(autostart) = config.autostart {
        interface.autostart = autostart;
    }
    if config.method.is_some() {
        interface.method = config.method;
    }
    if config.method6.is_some() {
        interface.method6 = config.method6;
    }
    if config.mtu.is_some() {
        interface.mtu = config.mtu;
    }
    if config.comments.is_some() {
        interface.comments = config.comments;
    }
    if config.comments6.is_some() {
        interface.comments6 = config.comments6;
    }

    if let Some(cidr) = config.cidr {
        let (_, _, is_v6) = network::parse_cidr(&cidr)?;
        if is_v6 {
            bail!("invalid address type (expected IPv4, got IPv6)");
        }
        interface.cidr = Some(cidr);
    }

    if let Some(cidr6) = config.cidr6 {
        let (_, _, is_v6) = network::parse_cidr(&cidr6)?;
        if !is_v6 {
            bail!("invalid address type (expected IPv6, got IPv4)");
        }
        interface.cidr6 = Some(cidr6);
    }

    if let Some(gateway) = config.gateway {
        let is_v6 = gateway.contains(':');
        if is_v6 {
            bail!("invalid address type (expected IPv4, got IPv6)");
        }
        network_config.check_duplicate_gateway_v4(&iface)?;
        interface.gateway = Some(gateway);
    }

    if let Some(gateway6) = config.gateway6 {
        let is_v6 = gateway6.contains(':');
        if !is_v6 {
            bail!("invalid address type (expected IPv6, got IPv4)");
        }
        network_config.check_duplicate_gateway_v6(&iface)?;
        interface.gateway6 = Some(gateway6);
    }

    match interface_type {
        NetworkInterfaceType::Bridge => {
            if let Some(ports) = &config.bridge_ports {
                interface.set_bridge_port_list(ports)?;
            }
            if config.bridge_vlan_aware.is_some() {
                interface.bridge_vlan_aware = config.bridge_vlan_aware;
            }
        }
        NetworkInterfaceType::Bond => {
            if let Some(mode) = config.bond_mode {
                interface.bond_mode = config.bond_mode;
                if config.bond_primary.is_some() {
                    if mode != LinuxBondMode::ActiveBackup {
                        bail!("bond-primary is only valid with Active/Backup mode");
                    }
                    interface.bond_primary = config.bond_primary;
                }
                if config.bond_xmit_hash_policy.is_some() {
                    if mode != LinuxBondMode::Ieee802_3ad && mode != LinuxBondMode::BalanceXor {
                        bail!("bond_xmit_hash_policy is only valid with LACP(802.3ad) or balance-xor mode");
                    }
                    interface.bond_xmit_hash_policy = config.bond_xmit_hash_policy;
                }
            }
            if let Some(slaves) = &config.slaves {
                interface.set_bond_slave_list(&slaves)?;
            }
        }
        NetworkInterfaceType::Vlan => {
            if config.vlan_id.is_none() && parse_vlan_id_from_name(&iface).is_none() {
                bail!("vlan-id must be set");
            }
            interface.vlan_id = config.vlan_id;

            if let Some(dev) = config
                .vlan_raw_device
                .as_deref()
                .or_else(|| parse_vlan_raw_device_from_name(&iface))
            {
                if !network_config.interfaces.contains_key(dev) {
                    bail!("vlan-raw-device {dev} does not exist");
                }
            } else {
                bail!("vlan-raw-device must be set");
            }
            interface.vlan_raw_device = config.vlan_raw_device;
        }
        _ => bail!(
            "creating network interface type '{:?}' is not supported",
            interface_type
        ),
    }

    if interface.cidr.is_some() || interface.gateway.is_some() {
        interface.method = Some(NetworkConfigMethod::Static);
    } else if interface.method.is_none() {
        interface.method = Some(NetworkConfigMethod::Manual);
    }

    if interface.cidr6.is_some() || interface.gateway6.is_some() {
        interface.method6 = Some(NetworkConfigMethod::Static);
    } else if interface.method6.is_none() {
        interface.method6 = Some(NetworkConfigMethod::Manual);
    }

    network_config.interfaces.insert(iface, interface);

    network::save_config(&network_config)?;

    Ok(())
}

/// Update network interface config.
pub fn update_interface(
    iface: String,
    update: InterfaceUpdater,
    delete: Option<Vec<DeletableInterfaceProperty>>,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    let _lock = network::lock_config()?;

    let (mut network_config, expected_digest) = network::config()?;

    expected_digest.detect_modification(digest.as_ref())?;

    if update.gateway.is_some() {
        network_config.check_duplicate_gateway_v4(&iface)?;
    }
    if update.gateway6.is_some() {
        network_config.check_duplicate_gateway_v6(&iface)?;
    }

    if let Some(dev) = update
        .vlan_raw_device
        .as_deref()
        .or_else(|| parse_vlan_raw_device_from_name(&iface))
    {
        if !network_config.interfaces.contains_key(dev) {
            bail!("vlan-raw-device {dev} does not exist");
        }
    }

    let interface = network_config.lookup_mut(&iface)?;

    if let Some(interface_type) = update.interface_type {
        if interface_type != interface.interface_type {
            bail!(
                "got unexpected interface type ({:?} != {:?})",
                interface_type,
                interface.interface_type
            );
        }
    }

    if let Some(delete) = delete {
        for delete_prop in delete {
            match delete_prop {
                DeletableInterfaceProperty::Cidr => {
                    interface.cidr = None;
                }
                DeletableInterfaceProperty::Cidr6 => {
                    interface.cidr6 = None;
                }
                DeletableInterfaceProperty::Gateway => {
                    interface.gateway = None;
                }
                DeletableInterfaceProperty::Gateway6 => {
                    interface.gateway6 = None;
                }
                DeletableInterfaceProperty::Method => {
                    interface.method = None;
                }
                DeletableInterfaceProperty::Method6 => {
                    interface.method6 = None;
                }
                DeletableInterfaceProperty::Comments => {
                    interface.comments = None;
                }
                DeletableInterfaceProperty::Comments6 => {
                    interface.comments6 = None;
                }
                DeletableInterfaceProperty::Mtu => {
                    interface.mtu = None;
                }
                DeletableInterfaceProperty::Autostart => {
                    interface.autostart = false;
                }
                DeletableInterfaceProperty::BridgePorts => {
                    interface.set_bridge_ports(Vec::new())?;
                }
                DeletableInterfaceProperty::BridgeVlanAware => {
                    interface.bridge_vlan_aware = None;
                }
                DeletableInterfaceProperty::Slaves => {
                    interface.set_bond_slaves(Vec::new())?;
                }
                DeletableInterfaceProperty::BondPrimary => {
                    interface.bond_primary = None;
                }
                DeletableInterfaceProperty::BondXmitHashPolicy => {
                    interface.bond_xmit_hash_policy = None
                }
            }
        }
    }

    if let Some(autostart) = update.autostart {
        interface.autostart = autostart;
    }
    if update.method.is_some() {
        interface.method = update.method;
    }
    if update.method6.is_some() {
        interface.method6 = update.method6;
    }
    if update.mtu.is_some() {
        interface.mtu = update.mtu;
    }
    if let Some(ports) = &update.bridge_ports {
        interface.set_bridge_port_list(ports)?;
    }
    if update.bridge_vlan_aware.is_some() {
        interface.bridge_vlan_aware = update.bridge_vlan_aware;
    }
    if let Some(slaves) = &update.slaves {
        interface.set_bond_slave_list(slaves)?;
    }
    if let Some(mode) = update.bond_mode {
        interface.bond_mode = update.bond_mode;
        if update.bond_primary.is_some() {
            if mode != LinuxBondMode::ActiveBackup {
                bail!("bond-primary is only valid with Active/Backup mode");
            }
            interface.bond_primary = update.bond_primary;
        }
        if update.bond_xmit_hash_policy.is_some() {
            if mode != LinuxBondMode::Ieee802_3ad && mode != LinuxBondMode::BalanceXor {
                bail!("bond_xmit_hash_policy is only valid with LACP(802.3ad) or balance-xor mode");
            }
            interface.bond_xmit_hash_policy = update.bond_xmit_hash_policy;
        }
    }

    if let Some(cidr) = update.cidr {
        let (_, _, is_v6) = network::parse_cidr(&cidr)?;
        if is_v6 {
            bail!("invalid address type (expected IPv4, got IPv6)");
        }
        interface.cidr = Some(cidr);
    }

    if let Some(cidr6) = update.cidr6 {
        let (_, _, is_v6) = network::parse_cidr(&cidr6)?;
        if !is_v6 {
            bail!("invalid address type (expected IPv6, got IPv4)");
        }
        interface.cidr6 = Some(cidr6);
    }

    if let Some(gateway) = update.gateway {
        let is_v6 = gateway.contains(':');
        if is_v6 {
            bail!("invalid address type (expected IPv4, got IPv6)");
        }
        interface.gateway = Some(gateway);
    }

    if let Some(gateway6) = update.gateway6 {
        let is_v6 = gateway6.contains(':');
        if !is_v6 {
            bail!("invalid address type (expected IPv6, got IPv4)");
        }
        interface.gateway6 = Some(gateway6);
    }

    if update.comments.is_some() {
        interface.comments = update.comments;
    }
    if update.comments6.is_some() {
        interface.comments6 = update.comments6;
    }

    if interface.cidr.is_some() || interface.gateway.is_some() {
        interface.method = Some(NetworkConfigMethod::Static);
    } else {
        interface.method = Some(NetworkConfigMethod::Manual);
    }

    if interface.cidr6.is_some() || interface.gateway6.is_some() {
        interface.method6 = Some(NetworkConfigMethod::Static);
    } else {
        interface.method6 = Some(NetworkConfigMethod::Manual);
    }

    if update.vlan_id.is_some() {
        interface.vlan_id = update.vlan_id;
    }
    if update.vlan_raw_device.is_some() {
        interface.vlan_raw_device = update.vlan_raw_device;
    }

    network::save_config(&network_config)?;

    Ok(())
}
