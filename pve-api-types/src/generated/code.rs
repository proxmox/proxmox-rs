/// PVE API client
/// Note that the following API URLs are not handled currently:
///
/// ```text
/// - /access
/// - /access/acl
/// - /access/domains/{realm}
/// - /access/domains/{realm}/sync
/// - /access/groups
/// - /access/groups/{groupid}
/// - /access/openid
/// - /access/openid/auth-url
/// - /access/openid/login
/// - /access/password
/// - /access/permissions
/// - /access/roles
/// - /access/roles/{roleid}
/// - /access/tfa
/// - /access/tfa/{userid}
/// - /access/tfa/{userid}/{id}
/// - /access/ticket
/// - /access/users
/// - /access/users/{userid}
/// - /access/users/{userid}/tfa
/// - /access/users/{userid}/token
/// - /access/users/{userid}/unlock-tfa
/// - /cluster
/// - /cluster/acme
/// - /cluster/acme/account
/// - /cluster/acme/account/{name}
/// - /cluster/acme/challenge-schema
/// - /cluster/acme/directories
/// - /cluster/acme/meta
/// - /cluster/acme/plugins
/// - /cluster/acme/plugins/{id}
/// - /cluster/acme/tos
/// - /cluster/backup
/// - /cluster/backup-info
/// - /cluster/backup-info/not-backed-up
/// - /cluster/backup/{id}
/// - /cluster/backup/{id}/included_volumes
/// - /cluster/ceph
/// - /cluster/ceph/flags
/// - /cluster/ceph/flags/{flag}
/// - /cluster/ceph/metadata
/// - /cluster/ceph/status
/// - /cluster/config
/// - /cluster/config/apiversion
/// - /cluster/config/nodes
/// - /cluster/config/nodes/{node}
/// - /cluster/config/qdevice
/// - /cluster/config/totem
/// - /cluster/firewall
/// - /cluster/firewall/aliases
/// - /cluster/firewall/aliases/{name}
/// - /cluster/firewall/groups
/// - /cluster/firewall/groups/{group}
/// - /cluster/firewall/groups/{group}/{pos}
/// - /cluster/firewall/ipset
/// - /cluster/firewall/ipset/{name}
/// - /cluster/firewall/ipset/{name}/{cidr}
/// - /cluster/firewall/macros
/// - /cluster/firewall/options
/// - /cluster/firewall/refs
/// - /cluster/firewall/rules
/// - /cluster/firewall/rules/{pos}
/// - /cluster/ha
/// - /cluster/ha/groups
/// - /cluster/ha/groups/{group}
/// - /cluster/ha/resources
/// - /cluster/ha/resources/{sid}
/// - /cluster/ha/resources/{sid}/migrate
/// - /cluster/ha/resources/{sid}/relocate
/// - /cluster/ha/status
/// - /cluster/ha/status/current
/// - /cluster/ha/status/manager_status
/// - /cluster/jobs
/// - /cluster/jobs/realm-sync
/// - /cluster/jobs/realm-sync/{id}
/// - /cluster/jobs/schedule-analyze
/// - /cluster/log
/// - /cluster/mapping
/// - /cluster/mapping/dir
/// - /cluster/mapping/dir/{id}
/// - /cluster/mapping/pci
/// - /cluster/mapping/pci/{id}
/// - /cluster/mapping/usb
/// - /cluster/mapping/usb/{id}
/// - /cluster/metrics
/// - /cluster/metrics/server
/// - /cluster/metrics/server/{id}
/// - /cluster/nextid
/// - /cluster/notifications
/// - /cluster/notifications/endpoints
/// - /cluster/notifications/endpoints/gotify
/// - /cluster/notifications/endpoints/gotify/{name}
/// - /cluster/notifications/endpoints/sendmail
/// - /cluster/notifications/endpoints/sendmail/{name}
/// - /cluster/notifications/endpoints/smtp
/// - /cluster/notifications/endpoints/smtp/{name}
/// - /cluster/notifications/endpoints/webhook
/// - /cluster/notifications/endpoints/webhook/{name}
/// - /cluster/notifications/matcher-field-values
/// - /cluster/notifications/matcher-fields
/// - /cluster/notifications/matchers
/// - /cluster/notifications/matchers/{name}
/// - /cluster/notifications/targets
/// - /cluster/notifications/targets/{name}
/// - /cluster/notifications/targets/{name}/test
/// - /cluster/options
/// - /cluster/replication
/// - /cluster/replication/{id}
/// - /cluster/sdn
/// - /cluster/sdn/controllers
/// - /cluster/sdn/controllers/{controller}
/// - /cluster/sdn/dns
/// - /cluster/sdn/dns/{dns}
/// - /cluster/sdn/ipams
/// - /cluster/sdn/ipams/{ipam}
/// - /cluster/sdn/ipams/{ipam}/status
/// - /cluster/sdn/vnets
/// - /cluster/sdn/vnets/{vnet}
/// - /cluster/sdn/vnets/{vnet}/firewall
/// - /cluster/sdn/vnets/{vnet}/firewall/options
/// - /cluster/sdn/vnets/{vnet}/firewall/rules
/// - /cluster/sdn/vnets/{vnet}/firewall/rules/{pos}
/// - /cluster/sdn/vnets/{vnet}/ips
/// - /cluster/sdn/vnets/{vnet}/subnets
/// - /cluster/sdn/vnets/{vnet}/subnets/{subnet}
/// - /cluster/sdn/zones
/// - /cluster/sdn/zones/{zone}
/// - /cluster/tasks
/// - /nodes/{node}
/// - /nodes/{node}/aplinfo
/// - /nodes/{node}/apt
/// - /nodes/{node}/apt/changelog
/// - /nodes/{node}/apt/repositories
/// - /nodes/{node}/apt/update
/// - /nodes/{node}/apt/versions
/// - /nodes/{node}/capabilities
/// - /nodes/{node}/capabilities/qemu
/// - /nodes/{node}/capabilities/qemu/cpu
/// - /nodes/{node}/capabilities/qemu/machines
/// - /nodes/{node}/ceph
/// - /nodes/{node}/ceph/cfg
/// - /nodes/{node}/ceph/cfg/db
/// - /nodes/{node}/ceph/cfg/raw
/// - /nodes/{node}/ceph/cfg/value
/// - /nodes/{node}/ceph/cmd-safety
/// - /nodes/{node}/ceph/crush
/// - /nodes/{node}/ceph/fs
/// - /nodes/{node}/ceph/fs/{name}
/// - /nodes/{node}/ceph/init
/// - /nodes/{node}/ceph/log
/// - /nodes/{node}/ceph/mds
/// - /nodes/{node}/ceph/mds/{name}
/// - /nodes/{node}/ceph/mgr
/// - /nodes/{node}/ceph/mgr/{id}
/// - /nodes/{node}/ceph/mon
/// - /nodes/{node}/ceph/mon/{monid}
/// - /nodes/{node}/ceph/osd
/// - /nodes/{node}/ceph/osd/{osdid}
/// - /nodes/{node}/ceph/osd/{osdid}/in
/// - /nodes/{node}/ceph/osd/{osdid}/lv-info
/// - /nodes/{node}/ceph/osd/{osdid}/metadata
/// - /nodes/{node}/ceph/osd/{osdid}/out
/// - /nodes/{node}/ceph/osd/{osdid}/scrub
/// - /nodes/{node}/ceph/pool
/// - /nodes/{node}/ceph/pool/{name}
/// - /nodes/{node}/ceph/pool/{name}/status
/// - /nodes/{node}/ceph/restart
/// - /nodes/{node}/ceph/rules
/// - /nodes/{node}/ceph/start
/// - /nodes/{node}/ceph/status
/// - /nodes/{node}/ceph/stop
/// - /nodes/{node}/certificates
/// - /nodes/{node}/certificates/acme
/// - /nodes/{node}/certificates/acme/certificate
/// - /nodes/{node}/certificates/custom
/// - /nodes/{node}/certificates/info
/// - /nodes/{node}/config
/// - /nodes/{node}/disks
/// - /nodes/{node}/disks/directory
/// - /nodes/{node}/disks/directory/{name}
/// - /nodes/{node}/disks/initgpt
/// - /nodes/{node}/disks/list
/// - /nodes/{node}/disks/lvm
/// - /nodes/{node}/disks/lvm/{name}
/// - /nodes/{node}/disks/lvmthin
/// - /nodes/{node}/disks/lvmthin/{name}
/// - /nodes/{node}/disks/smart
/// - /nodes/{node}/disks/wipedisk
/// - /nodes/{node}/disks/zfs
/// - /nodes/{node}/disks/zfs/{name}
/// - /nodes/{node}/dns
/// - /nodes/{node}/execute
/// - /nodes/{node}/firewall
/// - /nodes/{node}/firewall/log
/// - /nodes/{node}/firewall/options
/// - /nodes/{node}/firewall/rules
/// - /nodes/{node}/firewall/rules/{pos}
/// - /nodes/{node}/hardware
/// - /nodes/{node}/hardware/pci
/// - /nodes/{node}/hardware/pci/{pci-id-or-mapping}
/// - /nodes/{node}/hardware/pci/{pci-id-or-mapping}/mdev
/// - /nodes/{node}/hardware/usb
/// - /nodes/{node}/hosts
/// - /nodes/{node}/journal
/// - /nodes/{node}/lxc/{vmid}
/// - /nodes/{node}/lxc/{vmid}/clone
/// - /nodes/{node}/lxc/{vmid}/feature
/// - /nodes/{node}/lxc/{vmid}/firewall
/// - /nodes/{node}/lxc/{vmid}/firewall/aliases
/// - /nodes/{node}/lxc/{vmid}/firewall/aliases/{name}
/// - /nodes/{node}/lxc/{vmid}/firewall/ipset
/// - /nodes/{node}/lxc/{vmid}/firewall/ipset/{name}
/// - /nodes/{node}/lxc/{vmid}/firewall/ipset/{name}/{cidr}
/// - /nodes/{node}/lxc/{vmid}/firewall/log
/// - /nodes/{node}/lxc/{vmid}/firewall/options
/// - /nodes/{node}/lxc/{vmid}/firewall/refs
/// - /nodes/{node}/lxc/{vmid}/firewall/rules
/// - /nodes/{node}/lxc/{vmid}/firewall/rules/{pos}
/// - /nodes/{node}/lxc/{vmid}/interfaces
/// - /nodes/{node}/lxc/{vmid}/move_volume
/// - /nodes/{node}/lxc/{vmid}/mtunnel
/// - /nodes/{node}/lxc/{vmid}/mtunnelwebsocket
/// - /nodes/{node}/lxc/{vmid}/pending
/// - /nodes/{node}/lxc/{vmid}/resize
/// - /nodes/{node}/lxc/{vmid}/rrd
/// - /nodes/{node}/lxc/{vmid}/rrddata
/// - /nodes/{node}/lxc/{vmid}/snapshot
/// - /nodes/{node}/lxc/{vmid}/snapshot/{snapname}
/// - /nodes/{node}/lxc/{vmid}/snapshot/{snapname}/config
/// - /nodes/{node}/lxc/{vmid}/snapshot/{snapname}/rollback
/// - /nodes/{node}/lxc/{vmid}/spiceproxy
/// - /nodes/{node}/lxc/{vmid}/status
/// - /nodes/{node}/lxc/{vmid}/status/reboot
/// - /nodes/{node}/lxc/{vmid}/status/resume
/// - /nodes/{node}/lxc/{vmid}/status/suspend
/// - /nodes/{node}/lxc/{vmid}/template
/// - /nodes/{node}/lxc/{vmid}/termproxy
/// - /nodes/{node}/lxc/{vmid}/vncproxy
/// - /nodes/{node}/lxc/{vmid}/vncwebsocket
/// - /nodes/{node}/migrateall
/// - /nodes/{node}/netstat
/// - /nodes/{node}/network/{iface}
/// - /nodes/{node}/qemu/{vmid}
/// - /nodes/{node}/qemu/{vmid}/agent
/// - /nodes/{node}/qemu/{vmid}/agent/exec
/// - /nodes/{node}/qemu/{vmid}/agent/exec-status
/// - /nodes/{node}/qemu/{vmid}/agent/file-read
/// - /nodes/{node}/qemu/{vmid}/agent/file-write
/// - /nodes/{node}/qemu/{vmid}/agent/fsfreeze-freeze
/// - /nodes/{node}/qemu/{vmid}/agent/fsfreeze-status
/// - /nodes/{node}/qemu/{vmid}/agent/fsfreeze-thaw
/// - /nodes/{node}/qemu/{vmid}/agent/fstrim
/// - /nodes/{node}/qemu/{vmid}/agent/get-fsinfo
/// - /nodes/{node}/qemu/{vmid}/agent/get-host-name
/// - /nodes/{node}/qemu/{vmid}/agent/get-memory-block-info
/// - /nodes/{node}/qemu/{vmid}/agent/get-memory-blocks
/// - /nodes/{node}/qemu/{vmid}/agent/get-osinfo
/// - /nodes/{node}/qemu/{vmid}/agent/get-time
/// - /nodes/{node}/qemu/{vmid}/agent/get-timezone
/// - /nodes/{node}/qemu/{vmid}/agent/get-users
/// - /nodes/{node}/qemu/{vmid}/agent/get-vcpus
/// - /nodes/{node}/qemu/{vmid}/agent/info
/// - /nodes/{node}/qemu/{vmid}/agent/network-get-interfaces
/// - /nodes/{node}/qemu/{vmid}/agent/ping
/// - /nodes/{node}/qemu/{vmid}/agent/set-user-password
/// - /nodes/{node}/qemu/{vmid}/agent/shutdown
/// - /nodes/{node}/qemu/{vmid}/agent/suspend-disk
/// - /nodes/{node}/qemu/{vmid}/agent/suspend-hybrid
/// - /nodes/{node}/qemu/{vmid}/agent/suspend-ram
/// - /nodes/{node}/qemu/{vmid}/clone
/// - /nodes/{node}/qemu/{vmid}/cloudinit
/// - /nodes/{node}/qemu/{vmid}/cloudinit/dump
/// - /nodes/{node}/qemu/{vmid}/feature
/// - /nodes/{node}/qemu/{vmid}/firewall
/// - /nodes/{node}/qemu/{vmid}/firewall/aliases
/// - /nodes/{node}/qemu/{vmid}/firewall/aliases/{name}
/// - /nodes/{node}/qemu/{vmid}/firewall/ipset
/// - /nodes/{node}/qemu/{vmid}/firewall/ipset/{name}
/// - /nodes/{node}/qemu/{vmid}/firewall/ipset/{name}/{cidr}
/// - /nodes/{node}/qemu/{vmid}/firewall/log
/// - /nodes/{node}/qemu/{vmid}/firewall/options
/// - /nodes/{node}/qemu/{vmid}/firewall/refs
/// - /nodes/{node}/qemu/{vmid}/firewall/rules
/// - /nodes/{node}/qemu/{vmid}/firewall/rules/{pos}
/// - /nodes/{node}/qemu/{vmid}/monitor
/// - /nodes/{node}/qemu/{vmid}/move_disk
/// - /nodes/{node}/qemu/{vmid}/mtunnel
/// - /nodes/{node}/qemu/{vmid}/mtunnelwebsocket
/// - /nodes/{node}/qemu/{vmid}/pending
/// - /nodes/{node}/qemu/{vmid}/resize
/// - /nodes/{node}/qemu/{vmid}/rrd
/// - /nodes/{node}/qemu/{vmid}/rrddata
/// - /nodes/{node}/qemu/{vmid}/sendkey
/// - /nodes/{node}/qemu/{vmid}/snapshot
/// - /nodes/{node}/qemu/{vmid}/snapshot/{snapname}
/// - /nodes/{node}/qemu/{vmid}/snapshot/{snapname}/config
/// - /nodes/{node}/qemu/{vmid}/snapshot/{snapname}/rollback
/// - /nodes/{node}/qemu/{vmid}/spiceproxy
/// - /nodes/{node}/qemu/{vmid}/status
/// - /nodes/{node}/qemu/{vmid}/status/reboot
/// - /nodes/{node}/qemu/{vmid}/status/reset
/// - /nodes/{node}/qemu/{vmid}/status/resume
/// - /nodes/{node}/qemu/{vmid}/status/suspend
/// - /nodes/{node}/qemu/{vmid}/template
/// - /nodes/{node}/qemu/{vmid}/termproxy
/// - /nodes/{node}/qemu/{vmid}/unlink
/// - /nodes/{node}/qemu/{vmid}/vncproxy
/// - /nodes/{node}/qemu/{vmid}/vncwebsocket
/// - /nodes/{node}/query-url-metadata
/// - /nodes/{node}/replication
/// - /nodes/{node}/replication/{id}
/// - /nodes/{node}/replication/{id}/log
/// - /nodes/{node}/replication/{id}/schedule_now
/// - /nodes/{node}/replication/{id}/status
/// - /nodes/{node}/report
/// - /nodes/{node}/rrd
/// - /nodes/{node}/rrddata
/// - /nodes/{node}/scan
/// - /nodes/{node}/scan/cifs
/// - /nodes/{node}/scan/glusterfs
/// - /nodes/{node}/scan/iscsi
/// - /nodes/{node}/scan/lvm
/// - /nodes/{node}/scan/lvmthin
/// - /nodes/{node}/scan/nfs
/// - /nodes/{node}/scan/pbs
/// - /nodes/{node}/scan/zfs
/// - /nodes/{node}/sdn
/// - /nodes/{node}/sdn/zones
/// - /nodes/{node}/sdn/zones/{zone}
/// - /nodes/{node}/sdn/zones/{zone}/content
/// - /nodes/{node}/services
/// - /nodes/{node}/services/{service}
/// - /nodes/{node}/services/{service}/reload
/// - /nodes/{node}/services/{service}/restart
/// - /nodes/{node}/services/{service}/start
/// - /nodes/{node}/services/{service}/state
/// - /nodes/{node}/services/{service}/stop
/// - /nodes/{node}/spiceshell
/// - /nodes/{node}/startall
/// - /nodes/{node}/stopall
/// - /nodes/{node}/storage/{storage}
/// - /nodes/{node}/storage/{storage}/content
/// - /nodes/{node}/storage/{storage}/content/{volume}
/// - /nodes/{node}/storage/{storage}/download-url
/// - /nodes/{node}/storage/{storage}/file-restore
/// - /nodes/{node}/storage/{storage}/file-restore/download
/// - /nodes/{node}/storage/{storage}/file-restore/list
/// - /nodes/{node}/storage/{storage}/import-metadata
/// - /nodes/{node}/storage/{storage}/prunebackups
/// - /nodes/{node}/storage/{storage}/rrd
/// - /nodes/{node}/storage/{storage}/rrddata
/// - /nodes/{node}/storage/{storage}/status
/// - /nodes/{node}/storage/{storage}/upload
/// - /nodes/{node}/suspendall
/// - /nodes/{node}/syslog
/// - /nodes/{node}/termproxy
/// - /nodes/{node}/time
/// - /nodes/{node}/version
/// - /nodes/{node}/vncshell
/// - /nodes/{node}/vncwebsocket
/// - /nodes/{node}/vzdump
/// - /nodes/{node}/vzdump/defaults
/// - /nodes/{node}/vzdump/extractconfig
/// - /nodes/{node}/wakeonlan
/// - /pools
/// - /pools/{poolid}
/// - /storage
/// - /storage/{storage}
/// ```
#[async_trait::async_trait]
pub trait PveClient {
    /// Get information needed to join this cluster over the connected node.
    async fn cluster_config_join(&self, node: Option<String>) -> Result<ClusterJoinInfo, Error> {
        Err(Error::Other("cluster_config_join not implemented"))
    }

    /// Retrieve metrics of the cluster.
    async fn cluster_metrics_export(
        &self,
        history: Option<bool>,
        local_only: Option<bool>,
        start_time: Option<i64>,
    ) -> Result<ClusterMetrics, Error> {
        Err(Error::Other("cluster_metrics_export not implemented"))
    }

    /// Resources index (cluster wide).
    async fn cluster_resources(
        &self,
        ty: Option<ClusterResourceKind>,
    ) -> Result<Vec<ClusterResource>, Error> {
        Err(Error::Other("cluster_resources not implemented"))
    }

    /// Get cluster status information.
    async fn cluster_status(&self) -> Result<Vec<ClusterNodeStatus>, Error> {
        Err(Error::Other("cluster_status not implemented"))
    }

    /// Generate a new API token for a specific user. NOTE: returns API token
    /// value, which needs to be stored as it cannot be retrieved afterwards!
    async fn create_token(
        &self,
        userid: &str,
        tokenid: &str,
        params: CreateToken,
    ) -> Result<CreateTokenResponse, Error> {
        Err(Error::Other("create_token not implemented"))
    }

    /// Read subscription info.
    async fn get_subscription(&self, node: &str) -> Result<NodeSubscriptionInfo, Error> {
        Err(Error::Other("get_subscription not implemented"))
    }

    /// Read task list for one node (finished tasks).
    async fn get_task_list(
        &self,
        node: &str,
        params: ListTasks,
    ) -> Result<Vec<ListTasksResponse>, Error> {
        Err(Error::Other("get_task_list not implemented"))
    }

    /// Read task log.
    async fn get_task_log(
        &self,
        node: &str,
        upid: &str,
        download: Option<bool>,
        limit: Option<u64>,
        start: Option<u64>,
    ) -> Result<ApiResponseData<Vec<TaskLogLine>>, Error> {
        Err(Error::Other("get_task_log not implemented"))
    }

    /// Read task status.
    async fn get_task_status(&self, node: &str, upid: &str) -> Result<TaskStatus, Error> {
        Err(Error::Other("get_task_status not implemented"))
    }

    /// Authentication domain index.
    async fn list_domains(&self) -> Result<Vec<ListRealm>, Error> {
        Err(Error::Other("list_domains not implemented"))
    }

    /// LXC container index (per node).
    async fn list_lxc(&self, node: &str) -> Result<Vec<LxcEntry>, Error> {
        Err(Error::Other("list_lxc not implemented"))
    }

    /// List available networks
    async fn list_networks(
        &self,
        node: &str,
        ty: Option<ListNetworksType>,
    ) -> Result<Vec<NetworkInterface>, Error> {
        Err(Error::Other("list_networks not implemented"))
    }

    /// Cluster node index.
    async fn list_nodes(&self) -> Result<Vec<ClusterNodeIndexResponse>, Error> {
        Err(Error::Other("list_nodes not implemented"))
    }

    /// Virtual machine index (per node).
    async fn list_qemu(&self, node: &str, full: Option<bool>) -> Result<Vec<VmEntry>, Error> {
        Err(Error::Other("list_qemu not implemented"))
    }

    /// Get status for all datastores.
    async fn list_storages(
        &self,
        node: &str,
        content: Option<Vec<StorageContent>>,
        enabled: Option<bool>,
        format: Option<bool>,
        storage: Option<String>,
        target: Option<String>,
    ) -> Result<Vec<StorageInfo>, Error> {
        Err(Error::Other("list_storages not implemented"))
    }

    /// Get container configuration.
    async fn lxc_get_config(
        &self,
        node: &str,
        vmid: u32,
        current: Option<bool>,
        snapshot: Option<String>,
    ) -> Result<LxcConfig, Error> {
        Err(Error::Other("lxc_get_config not implemented"))
    }

    /// Get virtual machine status.
    async fn lxc_get_status(&self, node: &str, vmid: u32) -> Result<LxcStatus, Error> {
        Err(Error::Other("lxc_get_status not implemented"))
    }

    /// Migrate the container to another node. Creates a new migration task.
    async fn migrate_lxc(
        &self,
        node: &str,
        vmid: u32,
        params: MigrateLxc,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("migrate_lxc not implemented"))
    }

    /// Migrate virtual machine. Creates a new migration task.
    async fn migrate_qemu(
        &self,
        node: &str,
        vmid: u32,
        params: MigrateQemu,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("migrate_qemu not implemented"))
    }

    /// Read node status
    async fn node_status(&self, node: &str) -> Result<NodeStatus, Error> {
        Err(Error::Other("node_status not implemented"))
    }

    /// Get the virtual machine configuration with pending configuration changes
    /// applied. Set the 'current' parameter to get the current configuration
    /// instead.
    async fn qemu_get_config(
        &self,
        node: &str,
        vmid: u32,
        current: Option<bool>,
        snapshot: Option<String>,
    ) -> Result<QemuConfig, Error> {
        Err(Error::Other("qemu_get_config not implemented"))
    }

    /// Get virtual machine status.
    async fn qemu_get_status(&self, node: &str, vmid: u32) -> Result<QemuStatus, Error> {
        Err(Error::Other("qemu_get_status not implemented"))
    }

    /// Get preconditions for migration.
    async fn qemu_migrate_preconditions(
        &self,
        node: &str,
        vmid: u32,
        target: Option<String>,
    ) -> Result<QemuMigratePreconditions, Error> {
        Err(Error::Other("qemu_migrate_preconditions not implemented"))
    }

    /// Migrate the container to another cluster. Creates a new migration task.
    /// EXPERIMENTAL feature!
    async fn remote_migrate_lxc(
        &self,
        node: &str,
        vmid: u32,
        params: RemoteMigrateLxc,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("remote_migrate_lxc not implemented"))
    }

    /// Migrate virtual machine to a remote cluster. Creates a new migration
    /// task. EXPERIMENTAL feature!
    async fn remote_migrate_qemu(
        &self,
        node: &str,
        vmid: u32,
        params: RemoteMigrateQemu,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("remote_migrate_qemu not implemented"))
    }

    /// Shutdown the container. This will trigger a clean shutdown of the
    /// container, see lxc-stop(1) for details.
    async fn shutdown_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: ShutdownLxc,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("shutdown_lxc_async not implemented"))
    }

    /// Shutdown virtual machine. This is similar to pressing the power button
    /// on a physical machine. This will send an ACPI event for the guest OS,
    /// which should then proceed to a clean shutdown.
    async fn shutdown_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: ShutdownQemu,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("shutdown_qemu_async not implemented"))
    }

    /// Start the container.
    async fn start_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: StartLxc,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("start_lxc_async not implemented"))
    }

    /// Start virtual machine.
    async fn start_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: StartQemu,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("start_qemu_async not implemented"))
    }

    /// Stop the container. This will abruptly stop all processes running in the
    /// container.
    async fn stop_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: StopLxc,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("stop_lxc_async not implemented"))
    }

    /// Stop virtual machine. The qemu process will exit immediately. This is
    /// akin to pulling the power plug of a running computer and may damage the
    /// VM data.
    async fn stop_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: StopQemu,
    ) -> Result<PveUpid, Error> {
        Err(Error::Other("stop_qemu_async not implemented"))
    }

    /// Stop a task.
    async fn stop_task(&self, node: &str, upid: &str) -> Result<(), Error> {
        Err(Error::Other("stop_task not implemented"))
    }

    /// API version details, including some parts of the global datacenter
    /// config.
    async fn version(&self) -> Result<VersionResponse, Error> {
        Err(Error::Other("version not implemented"))
    }
}

#[async_trait::async_trait]
impl<T> PveClient for PveClientImpl<T>
where
    T: HttpApiClient + Send + Sync,
    for<'a> <T as HttpApiClient>::ResponseFuture<'a>: Send,
{
    /// Get information needed to join this cluster over the connected node.
    async fn cluster_config_join(&self, node: Option<String>) -> Result<ClusterJoinInfo, Error> {
        let url = &ApiPathBuilder::new("/api2/extjs/cluster/config/join")
            .maybe_arg("node", &node)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Retrieve metrics of the cluster.
    async fn cluster_metrics_export(
        &self,
        history: Option<bool>,
        local_only: Option<bool>,
        start_time: Option<i64>,
    ) -> Result<ClusterMetrics, Error> {
        let url = &ApiPathBuilder::new("/api2/extjs/cluster/metrics/export")
            .maybe_bool_arg("history", history)
            .maybe_bool_arg("local-only", local_only)
            .maybe_arg("start-time", &start_time)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Resources index (cluster wide).
    async fn cluster_resources(
        &self,
        ty: Option<ClusterResourceKind>,
    ) -> Result<Vec<ClusterResource>, Error> {
        let url = &ApiPathBuilder::new("/api2/extjs/cluster/resources")
            .maybe_arg("type", &ty)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get cluster status information.
    async fn cluster_status(&self) -> Result<Vec<ClusterNodeStatus>, Error> {
        let url = "/api2/extjs/cluster/status";
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Generate a new API token for a specific user. NOTE: returns API token
    /// value, which needs to be stored as it cannot be retrieved afterwards!
    async fn create_token(
        &self,
        userid: &str,
        tokenid: &str,
        params: CreateToken,
    ) -> Result<CreateTokenResponse, Error> {
        let url = &format!("/api2/extjs/access/users/{userid}/token/{tokenid}");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Read subscription info.
    async fn get_subscription(&self, node: &str) -> Result<NodeSubscriptionInfo, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/subscription");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Read task list for one node (finished tasks).
    async fn get_task_list(
        &self,
        node: &str,
        params: ListTasks,
    ) -> Result<Vec<ListTasksResponse>, Error> {
        let ListTasks {
            errors: p_errors,
            limit: p_limit,
            since: p_since,
            source: p_source,
            start: p_start,
            statusfilter: p_statusfilter,
            typefilter: p_typefilter,
            until: p_until,
            userfilter: p_userfilter,
            vmid: p_vmid,
        } = params;

        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/tasks"))
            .maybe_bool_arg("errors", p_errors)
            .maybe_arg("limit", &p_limit)
            .maybe_arg("since", &p_since)
            .maybe_arg("source", &p_source)
            .maybe_arg("start", &p_start)
            .maybe_arg("statusfilter", &p_statusfilter)
            .maybe_arg("typefilter", &p_typefilter)
            .maybe_arg("until", &p_until)
            .maybe_arg("userfilter", &p_userfilter)
            .maybe_arg("vmid", &p_vmid)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Read task log.
    async fn get_task_log(
        &self,
        node: &str,
        upid: &str,
        download: Option<bool>,
        limit: Option<u64>,
        start: Option<u64>,
    ) -> Result<ApiResponseData<Vec<TaskLogLine>>, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/tasks/{upid}/log"))
            .maybe_bool_arg("download", download)
            .maybe_arg("limit", &limit)
            .maybe_arg("start", &start)
            .build();
        self.0.get(url).await?.expect_json()
    }

    /// Read task status.
    async fn get_task_status(&self, node: &str, upid: &str) -> Result<TaskStatus, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/tasks/{upid}/status");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Authentication domain index.
    async fn list_domains(&self) -> Result<Vec<ListRealm>, Error> {
        let url = "/api2/extjs/access/domains";
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// LXC container index (per node).
    async fn list_lxc(&self, node: &str) -> Result<Vec<LxcEntry>, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// List available networks
    async fn list_networks(
        &self,
        node: &str,
        ty: Option<ListNetworksType>,
    ) -> Result<Vec<NetworkInterface>, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/network"))
            .maybe_arg("type", &ty)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Cluster node index.
    async fn list_nodes(&self) -> Result<Vec<ClusterNodeIndexResponse>, Error> {
        let url = "/api2/extjs/nodes";
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Virtual machine index (per node).
    async fn list_qemu(&self, node: &str, full: Option<bool>) -> Result<Vec<VmEntry>, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/qemu"))
            .maybe_bool_arg("full", full)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get status for all datastores.
    async fn list_storages(
        &self,
        node: &str,
        content: Option<Vec<StorageContent>>,
        enabled: Option<bool>,
        format: Option<bool>,
        storage: Option<String>,
        target: Option<String>,
    ) -> Result<Vec<StorageInfo>, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/storage"))
            .maybe_list_arg("content", &content)
            .maybe_bool_arg("enabled", enabled)
            .maybe_bool_arg("format", format)
            .maybe_arg("storage", &storage)
            .maybe_arg("target", &target)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get container configuration.
    async fn lxc_get_config(
        &self,
        node: &str,
        vmid: u32,
        current: Option<bool>,
        snapshot: Option<String>,
    ) -> Result<LxcConfig, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/lxc/{vmid}/config"))
            .maybe_bool_arg("current", current)
            .maybe_arg("snapshot", &snapshot)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get virtual machine status.
    async fn lxc_get_status(&self, node: &str, vmid: u32) -> Result<LxcStatus, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/status/current");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Migrate the container to another node. Creates a new migration task.
    async fn migrate_lxc(
        &self,
        node: &str,
        vmid: u32,
        params: MigrateLxc,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/migrate");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Migrate virtual machine. Creates a new migration task.
    async fn migrate_qemu(
        &self,
        node: &str,
        vmid: u32,
        params: MigrateQemu,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/migrate");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Read node status
    async fn node_status(&self, node: &str) -> Result<NodeStatus, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/status");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get the virtual machine configuration with pending configuration changes
    /// applied. Set the 'current' parameter to get the current configuration
    /// instead.
    async fn qemu_get_config(
        &self,
        node: &str,
        vmid: u32,
        current: Option<bool>,
        snapshot: Option<String>,
    ) -> Result<QemuConfig, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/qemu/{vmid}/config"))
            .maybe_bool_arg("current", current)
            .maybe_arg("snapshot", &snapshot)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get virtual machine status.
    async fn qemu_get_status(&self, node: &str, vmid: u32) -> Result<QemuStatus, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/status/current");
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Get preconditions for migration.
    async fn qemu_migrate_preconditions(
        &self,
        node: &str,
        vmid: u32,
        target: Option<String>,
    ) -> Result<QemuMigratePreconditions, Error> {
        let url = &ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/qemu/{vmid}/migrate"))
            .maybe_arg("target", &target)
            .build();
        Ok(self.0.get(url).await?.expect_json()?.data)
    }

    /// Migrate the container to another cluster. Creates a new migration task.
    /// EXPERIMENTAL feature!
    async fn remote_migrate_lxc(
        &self,
        node: &str,
        vmid: u32,
        params: RemoteMigrateLxc,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/remote_migrate");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Migrate virtual machine to a remote cluster. Creates a new migration
    /// task. EXPERIMENTAL feature!
    async fn remote_migrate_qemu(
        &self,
        node: &str,
        vmid: u32,
        params: RemoteMigrateQemu,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/remote_migrate");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Shutdown the container. This will trigger a clean shutdown of the
    /// container, see lxc-stop(1) for details.
    async fn shutdown_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: ShutdownLxc,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/status/shutdown");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Shutdown virtual machine. This is similar to pressing the power button
    /// on a physical machine. This will send an ACPI event for the guest OS,
    /// which should then proceed to a clean shutdown.
    async fn shutdown_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: ShutdownQemu,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/status/shutdown");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Start the container.
    async fn start_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: StartLxc,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/status/start");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Start virtual machine.
    async fn start_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: StartQemu,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/status/start");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Stop the container. This will abruptly stop all processes running in the
    /// container.
    async fn stop_lxc_async(
        &self,
        node: &str,
        vmid: u32,
        params: StopLxc,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/lxc/{vmid}/status/stop");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Stop virtual machine. The qemu process will exit immediately. This is
    /// akin to pulling the power plug of a running computer and may damage the
    /// VM data.
    async fn stop_qemu_async(
        &self,
        node: &str,
        vmid: u32,
        params: StopQemu,
    ) -> Result<PveUpid, Error> {
        let url = &format!("/api2/extjs/nodes/{node}/qemu/{vmid}/status/stop");
        Ok(self.0.post(url, &params).await?.expect_json()?.data)
    }

    /// Stop a task.
    async fn stop_task(&self, node: &str, upid: &str) -> Result<(), Error> {
        let url = &format!("/api2/extjs/nodes/{node}/tasks/{upid}");
        self.0.delete(url).await?.nodata()
    }

    /// API version details, including some parts of the global datacenter
    /// config.
    async fn version(&self) -> Result<VersionResponse, Error> {
        let url = "/api2/extjs/version";
        Ok(self.0.get(url).await?.expect_json()?.data)
    }
}
