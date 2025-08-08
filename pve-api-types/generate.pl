#!/usr/bin/env perl

use strict;
use warnings;

use Carp;
use IPC::Open2;

use Data::Dumper;
$Data::Dumper::Indent = 1;

# Load api components
use PVE::API2;
use PVE::API2::AccessControl;
use PVE::API2::Nodes;
use PVE::API2::NodeConfig;

# This is used to build the pve-storage-content enum:
use PVE::Storage::Plugin;

use lib './generator-lib';
use Schema2Rust;

my $output_dir = shift(@ARGV) // die "usage: $0 <output-directory>\n";

sub sq : prototype($) {
    return Schema2Rust::quote_string($_[0]);
}

# Dump api:
my $__API_ROOT = PVE::API2->api_dump(undef, 1);

# Initialize:
Schema2Rust::init_api($__API_ROOT);

# From JSONSchema.pm, but we can't use perl-re directly, particularly `qr//`...
my $CONFIGID_RE = '^(?i:[a-z][a-z0-9_-]+)$';

# Disable `#[api]` generation for now, it's incomplete/untested.
#$Schema2Rust::API = 0;

Schema2Rust::register_format('CIDR' => { code => 'verifiers::verify_cidr' });
Schema2Rust::register_format('CIDRv4' => { code => 'verifiers::verify_cidrv4' });
Schema2Rust::register_format('CIDRv6' => { code => 'verifiers::verify_cidrv6' });
Schema2Rust::register_format('ipv4mask' => { code => 'verifiers::verify_ipv4_mask' });
Schema2Rust::register_format('mac-addr' => { regex => '^(?i)[a-f0-9][02468ace](?::[a-f0-9]{2}){5}$' });
## Schema2Rust::register_format('pve-acme-alias' => { code => 'verify_pve_acme_alias' });
## Schema2Rust::register_format('pve-acme-domain' => { code => 'verify_pve_acme_domain' });
Schema2Rust::register_format('pve-bridge-id' => { regex => '^[-_.\w\d]+$' });
Schema2Rust::register_format('pve-configid' => { regex => $CONFIGID_RE });
## Schema2Rust::register_format('pve-groupid' => { code => 'verify_pve_groupid' });
Schema2Rust::register_format('pve-userid' => { code => 'verify_pve_userid' });
## # copied from JSONSchema's verify_pve_node sub:
Schema2Rust::register_format('pve-node' => { regex => '^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$' });
## #Schema2Rust::register_format('pve-node' => { code => 'verify_pve_node' });
## Schema2Rust::register_format('pve-priv' => { code => 'verify_pve_privileges' });
## Schema2Rust::register_format('pve-realm' => { code => 'verify_pve_realm' });
##
Schema2Rust::register_format('disk-size' => { regex => '^(\d+(\.\d+)?)([KMGT])?$' });
Schema2Rust::register_format('dns-name' => { code => 'verifiers::verify_dns_name' });
## Schema2Rust::register_format('email' => { code => 'verify_email' });
Schema2Rust::register_format('pve-phys-bits' => { code => 'verifiers::verify_pve_phys_bits' });
Schema2Rust::register_format('pve-qm-bootdev' => { unchecked => 1 });
Schema2Rust::register_format('pve-qm-bootdisk' => { regex => '^(ide|sata|scsi|virtio|efidisk|tpmstate)\d+$' });
Schema2Rust::register_format('pve-qm-usb-device' => { unchecked => 1 });
Schema2Rust::register_format('pve-startup-order' => { unchecked => 1 });
Schema2Rust::register_format('pve-storage-id' => { regex => '^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$' });
Schema2Rust::register_format('pve-storage-content' => { type => 'StorageContent' });
Schema2Rust::register_format('pve-tag' => { regex => '^(?i)[a-z0-9_][a-z0-9_\-+.]*$' });
Schema2Rust::register_format('pve-volume-id' => { code => 'verifiers::verify_volume_id' });
Schema2Rust::register_format('pve-volume-id-or-qm-path' => { code => 'verifiers::verify_pve_volume_id_or_qm_path' });
## Schema2Rust::register_format('pve-volume-id-or-absolute-path' => { code => 'verify_pve_volume_id_or_absolute_path' });
Schema2Rust::register_format('urlencoded' => { regex => '^[-%a-zA-Z0-9_.!~*\'()]*$' });
Schema2Rust::register_format('pve-cpuset' => { regex => '^(\s*\d+(-\d+)?\s*)(,\s*\d+(-\d+)?\s*)?$' });
##
Schema2Rust::register_format('pve-lxc-mp-string' => { code => 'verifiers::verify_lxc_mp_string' });
## Schema2Rust::register_format('lxc-ip-with-ll-iface' => { regex => ['^(?i:', \'pdm_api_types::IPRE!()', ')$'] });
Schema2Rust::register_format('lxc-ip-with-ll-iface' => { code => 'verifiers::verify_ip_with_ll_iface' });
Schema2Rust::register_format('pve-ct-timezone' => { regex => '^.*/.*$' });
Schema2Rust::register_format('pve-lxc-dev-string' => { code => 'verifiers::verify_pve_lxc_dev_string' });
##
Schema2Rust::register_format('storage-pair' => { code => 'verifiers::verify_storage_pair' });
Schema2Rust::register_format('bridge-pair' => { code => 'verifiers::verify_bridge_pair' });

Schema2Rust::register_format('pve-task-status-type' => { regex => '^(?i:ok|error|warning|unknown)$' });

Schema2Rust::register_enum_variant('PveVmCpuConfReportedModel::486' => 'I486');
Schema2Rust::register_enum_variant('QemuConfigEfidisk0Efitype::2m' => 'Mb2');
Schema2Rust::register_enum_variant('QemuConfigEfidisk0Efitype::4m' => 'Mb4');
Schema2Rust::register_enum_variant('QemuConfigHugepages::2' => 'Mb2');
Schema2Rust::register_enum_variant('QemuConfigHugepages::1024' => 'Mb1024');
Schema2Rust::register_enum_variant('PveQmRngSource::/dev/urandom', => 'DevUrandom');
Schema2Rust::register_enum_variant('PveQmRngSource::/dev/random', => 'DevRandom');
Schema2Rust::register_enum_variant('PveQmRngSource::/dev/hwrng', => 'DevHwrng');
Schema2Rust::register_enum_variant('QemuConfigTpmstate0Version::v1.2' => 'V1_2');
Schema2Rust::register_enum_variant('QemuConfigTpmstate0Version::v2.0' => 'V2_0');

## # FIXME: Invent an enum list type for this one
Schema2Rust::register_format('pve-hotplug-features' => { unchecked => 1 });
## # FIXME: Figure out something sane for these
Schema2Rust::register_format('address' => { code => 'verifiers::verify_address' });
Schema2Rust::register_format('ip' => { code => 'verifiers::verify_ip' });
Schema2Rust::register_format('ipv4' => { code => 'verifiers::verify_ipv4' });
Schema2Rust::register_format('ipv6' => { code => 'verifiers::verify_ipv6' });
Schema2Rust::register_format('pve-ipv4-config' => { code => 'verifiers::verify_ipv4_config' });
Schema2Rust::register_format('pve-ipv6-config' => { code => 'verifiers::verify_ipv6_config' });

Schema2Rust::register_format('pve-iface' => { regex => '^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$' });

Schema2Rust::register_format('pve-vlan-id-or-range' => { code => 'verifiers::verify_vlan_id_or_range' });

# This is used as both a task status and guest status.
Schema2Rust::generate_enum('IsRunning', {
    type => 'string',
    description => "A guest's run state.",
    enum => ['running', 'stopped'],
});

# We have a textual description of the default value in there, just pick the cgroupv2 one:
Schema2Rust::register_api_override('QemuConfig', '/properties/cpuunits/default', 1024);
Schema2Rust::register_api_override('LxcConfig', '/properties/cpuunits/default', 1024);
Schema2Rust::register_api_extension('LxcConfig', '/properties/lxc/items', {
    description => sq('A raw lxc config entry'),
});
Schema2Rust::register_api_extension('LxcConfig', '/properties/lxc/items/items', {
    description => sq('A config key value pair'),
});
Schema2Rust::register_api_override('StartQemu', '/properties/timeout/default', 30);
Schema2Rust::register_api_override('RemoteMigrateQemu', '/properties/bwlimit/default', undef);
Schema2Rust::register_api_override('RemoteMigrateLxc', '/properties/bwlimit/default', undef);

# Token api is missing some descriptions and has textual defaults for integers
Schema2Rust::register_api_extensions('CreateTokenResponseInfo', {
    '/properties/comment' => { description => sq("Description of the Token") },
});
Schema2Rust::register_api_extensions('CreateToken', {
    '/properties/comment' => { description => sq("Description of the Token") },
});
Schema2Rust::register_api_override('CreateTokenResponseInfo', '/properties/expire/default', undef);
Schema2Rust::register_api_override('CreateToken', '/properties/expire/default', undef);

# The task API is missing most documentation...
Schema2Rust::register_api_extensions('TaskStatus', {
    '/properties/exitstatus' => { description => sq("The task's exit status.") },
    '/properties/id' => { description => sq("The task id.") },
    '/properties/node' => { description => sq("The task's node.") },
    '/properties/type' => { description => sq("The task type.") },
    '/properties/upid' => { description => sq("The task's UPID.") },
    '/properties/user' => { description => sq("The task owner's user id.") },
    '/properties/pid' => { description => sq("The task process id.") },
    '/properties/pstart' => { description => sq("The task's proc start time.") },
    '/properties/starttime' => { description => sq("The task's start time.") },
});
Schema2Rust::register_api_extensions('ListTasksResponse', {
    '/properties/endtime' => { description => sq("The task's end time.") },
    '/properties/id' => { description => sq("The task id.") },
    '/properties/node' => { description => sq("The task's node.") },
    '/properties/pid' => { description => sq("The task process id.") },
    '/properties/pstart' => { description => sq("The task's proc start time.") },
    '/properties/starttime' => { description => sq("The task's start time.") },
    '/properties/status' => { description => sq("The task's status.") },
    '/properties/type' => { description => sq("The task type.") },
    '/properties/upid' => { description => sq("The task's UPID.") },
    '/properties/user' => { description => sq("The task owner's user id.") },
});
Schema2Rust::register_api_extensions('ClusterResource', {
    '/properties/id' => { description => sq("Resource id.") },
});

# pve-storage-content uses verify_
my $storage_content_types = [sort keys PVE::Storage::Plugin::valid_content_types('dir')->%*];
Schema2Rust::generate_enum('StorageContent', {
    type => 'string',
    description => 'Storage content type.',
    enum => $storage_content_types,
});

sub api : prototype($$$;%) {
    my ($method, $api_url, $rust_method_name, %extra) = @_;
    return Schema2Rust::api($method, $api_url, $rust_method_name, %extra);
}

# FIXME: this needs the return schema specified first:
api(GET => '/version', 'version', 'return-name' => 'VersionResponse');

# Deal with 'type' in `/cluster/resources` being different between input and output.
Schema2Rust::generate_enum(
    'ClusterResourceKind',
    {
        type => 'string',
        description => 'Resource type.',
        enum => ['vm', 'storage', 'node', 'sdn'],
    }
);
api(GET => '/cluster/resources', 'cluster_resources', 'return-name' => 'ClusterResource');

api(GET => '/nodes', 'list_nodes', 'return-name' => 'ClusterNodeIndexResponse');
Schema2Rust::derive('ClusterNodeIndexResponse' => 'Clone', 'PartialEq');
# api(
#     GET => '/nodes/{node}/config',
#     'get_node_config',
#     'param-name' => 'GetNodeConfig',
#     'return-name' => 'NodeConfig',
#     # 'return-type' => { type => 'object', properties => PVE::NodeConfig::get_nodeconfig_schema() },
# );
# api(PUT => '/nodes/{node}/config', 'set_node_config', 'param-name' => 'UpdateNodeConfig');
# subscription api
#
Schema2Rust::register_enum_variant('NodeSubscriptionInfoStatus::notfound' => 'NotFound');
api(GET => '/nodes/{node}/subscription', 'get_subscription',   'return-name' => 'NodeSubscriptionInfo');
# # low level task api:
# # ?? api(GET    => '/nodes/{node}/tasks/{upid}', 'get_task');
api(GET => '/nodes/{node}/tasks',               'get_task_list',   'param-name' => 'ListTasks');
Schema2Rust::derive('ListTasks' => 'Default');
api(GET => '/nodes/{node}/tasks/{upid}/status', 'get_task_status', 'return-name' => 'TaskStatus');
api(GET => '/nodes/{node}/tasks/{upid}/log',    'get_task_log',    'return-name' => 'TaskLogLine', attribs => 1);
api(DELETE => '/nodes/{node}/tasks/{upid}',     'stop_task');

api(GET => '/nodes/{node}/qemu', 'list_qemu', 'param-name' => 'FixmeListQemu', 'return-name' => 'VmEntry');
api(GET => '/nodes/{node}/qemu/{vmid}/config', 'qemu_get_config', 'param-name' => 'FixmeQemuGetConfig', 'return-name' => 'QemuConfig');
api(GET => '/nodes/{node}/qemu/{vmid}/status/current', 'qemu_get_status', 'return-name' => 'QemuStatus');
# api(POST => '/nodes/{node}/qemu/{vmid}/config', 'qemu_update_config_async', 'param-name' => 'UpdateQemuConfig');
api(POST => '/nodes/{node}/qemu/{vmid}/status/start',    'start_qemu_async',    'output-type' => 'PveUpid', 'param-name' => 'StartQemu');
api(POST => '/nodes/{node}/qemu/{vmid}/status/stop',     'stop_qemu_async',     'output-type' => 'PveUpid', 'param-name' => 'StopQemu');
api(POST => '/nodes/{node}/qemu/{vmid}/status/shutdown', 'shutdown_qemu_async', 'output-type' => 'PveUpid', 'param-name' => 'ShutdownQemu');
api(GET => '/nodes/{node}/qemu/{vmid}/migrate',           'qemu_migrate_preconditions', 'return-name' => 'QemuMigratePreconditions');
Schema2Rust::derive('QemuMigratePreconditionsNotAllowedNodesBlockingHaResources' => 'Clone', 'PartialEq');
Schema2Rust::derive('QemuMigratePreconditionsNotAllowedNodes' => 'Clone', 'PartialEq');
Schema2Rust::derive('QemuMigratePreconditionsLocalDisks' => 'Clone', 'PartialEq');
Schema2Rust::derive('QemuMigratePreconditions' => 'Clone', 'PartialEq');

Schema2Rust::derive('StartQemu' => 'Default');
Schema2Rust::derive('StopQemu' => 'Default');
Schema2Rust::derive('ShutdownQemu' => 'Default');
api(POST => '/nodes/{node}/qemu/{vmid}/migrate',        'migrate_qemu',         'output-type' => 'PveUpid', 'param-name' => 'MigrateQemu');
Schema2Rust::register_api_override('MigrateQemu', '/properties/bwlimit/default', undef);
api(POST => '/nodes/{node}/qemu/{vmid}/remote_migrate', 'remote_migrate_qemu',  'output-type' => 'PveUpid', 'param-name' => 'RemoteMigrateQemu');

api(GET => '/nodes/{node}/lxc',                         'list_lxc',            'param-name' => 'FixmeListLxc',      'return-name' => 'LxcEntry');
api(GET => '/nodes/{node}/lxc/{vmid}/config',           'lxc_get_config',      'param-name' => 'FixmeLxcGetConfig', 'return-name' => 'LxcConfig');
api(GET => '/nodes/{node}/lxc/{vmid}/status/current',   'lxc_get_status',      'return-name' => 'LxcStatus');
api(POST => '/nodes/{node}/lxc/{vmid}/status/start',    'start_lxc_async',     'output-type' => 'PveUpid', 'param-name' => 'StartLxc');
api(POST => '/nodes/{node}/lxc/{vmid}/status/stop',     'stop_lxc_async',      'output-type' => 'PveUpid', 'param-name' => 'StopLxc');
api(POST => '/nodes/{node}/lxc/{vmid}/status/shutdown', 'shutdown_lxc_async',  'output-type' => 'PveUpid', 'param-name' => 'ShutdownLxc');
Schema2Rust::derive('StartLxc' => 'Default');
Schema2Rust::derive('StopLxc' => 'Default');
Schema2Rust::derive('ShutdownLxc' => 'Default');
api(POST => '/nodes/{node}/lxc/{vmid}/migrate',        'migrate_lxc',         'output-type' => 'PveUpid', 'param-name' => 'MigrateLxc');
Schema2Rust::register_api_override('MigrateLxc', '/properties/bwlimit/default', undef);
api(POST => '/nodes/{node}/lxc/{vmid}/remote_migrate', 'remote_migrate_lxc',  'output-type' => 'PveUpid', 'param-name' => 'RemoteMigrateLxc');

Schema2Rust::register_enum_variant('ListNetworksType::OVSBridge' => 'OvsBridge');
Schema2Rust::register_enum_variant('ListNetworksType::OVSBond' => 'OvsBond');
Schema2Rust::register_enum_variant('ListNetworksType::OVSPort' => 'OvsPort');
Schema2Rust::register_enum_variant('ListNetworksType::OVSIntPort' => 'OvsIntPort');
Schema2Rust::register_enum_variant('NetworkInterfaceBondXmitHashPolicy::layer2+3' => 'Layer2_3');
Schema2Rust::register_enum_variant('NetworkInterfaceBondXmitHashPolicy::layer3+4' => 'Layer3_4');
Schema2Rust::register_enum_variant('NetworkInterfaceBondMode::802.3ad' => 'Ieee802_3ad');
Schema2Rust::register_enum_variant('NetworkInterfaceVlanProtocol::802.1ad' => 'Ieee802_1ad');
Schema2Rust::register_enum_variant('NetworkInterfaceVlanProtocol::802.1q' => 'Ieee802_1q');
api(GET => '/nodes/{node}/network', 'list_networks', 'return-name' => 'NetworkInterface');
Schema2Rust::derive('NetworkInterface' => 'Clone', 'PartialEq');

api(GET => '/nodes/{node}/storage', 'list_storages', 'return-name' => 'StorageInfo');
Schema2Rust::derive('StorageInfo' => 'Clone', 'PartialEq');

api(GET => '/nodes/{node}/status', 'node_status', 'return-name' => 'NodeStatus');

Schema2Rust::register_api_override('ClusterMetrics', '/properties/data/items', { type => "ClusterMetricsData"});
api(GET => '/cluster/metrics/export', 'cluster_metrics_export', 'return-name' => 'ClusterMetrics');

Schema2Rust::register_api_extensions('ClusterJoinInfoNodelist', {
    '/properties/pve_addr' => { description => sq("FIXME: Missing description in PVE.") },
    '/properties/pve_fp' => { description => sq("FIXME: Missing description in PVE.") },
    '/properties/quorum_votes' => { description => sq("FIXME: Missing description in PVE.") },
});
Schema2Rust::register_api_extensions('ClusterJoinInfo', {
    '/properties/config_digest' => { description => sq("FIXME: Missing description in PVE.") },
    '/properties/nodelist' => { description => sq("FIXME: Missing description in PVE.") },
});
api(GET => '/cluster/config/join', 'cluster_config_join', 'return-name' => 'ClusterJoinInfo');

# cluster status info
Schema2Rust::register_api_extensions('ClusterNodeStatus', {
    '/properties/id' => { description => sq("FIXME: Missing description in PVE.") },
    '/properties/name' => { description => sq("FIXME: Missing description in PVE.") },
});
api(GET => '/cluster/status', 'cluster_status', 'return-name' => 'ClusterNodeStatus');

# api(GET => '/storage', 'list_storages', 'return-name' => 'StorageList');
Schema2Rust::register_api_extensions('ListRealm', {
    '/properties/realm' => { description => sq("FIXME: Missing description in PVE.") },
    '/properties/type' => { description => sq("FIXME: Missing description in PVE.") },
});
api(GET => '/access/domains', 'list_domains', 'return-name' => 'ListRealm');
Schema2Rust::derive('ListRealm' => 'Clone', 'PartialEq');
# api(GET => '/access/groups', 'list_groups', 'return-name' => 'ListGroups');
# api(GET => '/access/groups/{groupid}', 'get_group', 'return-name' => 'Group');
# api(GET => '/access/users', 'list_users', 'return-name' => 'ListUsers');
# api(GET => '/access/users/{userid}', 'get_user', 'return-name' => 'User');
api(POST => '/access/users/{userid}/token/{tokenid}', 'create_token', 'param-name' => 'CreateToken');
Schema2Rust::derive('CreateToken' => 'Default');

# NOW DUMP THE CODE:
#
# We generate one file for API types, and one for API method calls.

my $type_out_file = "$output_dir/types.rs";
my $code_out_file = "$output_dir/code.rs";

# Redirect code generation through rustfmt:
open(my $type_fh, '>', $type_out_file) or die "failed to open '$type_out_file': $!\n";
my $type_pid = open2(
    '>&'.fileno($type_fh),
    my $type_pipe,
    #'cat',
    'rustfmt', '--edition=2021', '--config', 'wrap_comments=true'
);
open(my $code_fh, '>', $code_out_file) or die "failed to open '$code_out_file': $!\n";
my $code_pid = open2(
    '>&'.fileno($code_fh),
    my $code_pipe,
    #'cat',
    'rustfmt', '--edition=2021', '--config', 'wrap_comments=true'
);
close($type_fh);
close($code_fh);

# Create .rs files:
print {$code_pipe} "/// PVE API client\n";
print {$code_pipe} "/// Note that the following API URLs are not handled currently:\n";
print {$code_pipe} "///\n";
print {$code_pipe} "/// ```text\n";
my $unused = Schema2Rust::get_unused_paths();
for my $path (sort keys $unused->%*) {
    print {$code_pipe} "/// - $path\n";
}
print {$code_pipe} "/// ```\n";

# Schema2Rust::dump();
Schema2Rust::print_types($type_pipe);
Schema2Rust::print_trait($code_pipe);
print {$code_pipe} "\n";
Schema2Rust::print_implementation($code_pipe);
$type_pipe->flush();
$code_pipe->flush();
close($type_pipe);
close($code_pipe);

# Wait for formatters to finish:
do {} while $type_pid != waitpid($type_pid, 0);
do {} while $code_pid != waitpid($code_pid, 0);
