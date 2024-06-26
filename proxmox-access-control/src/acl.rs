use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{bail, Error};

use proxmox_auth_api::types::{Authid, Userid};
use proxmox_product_config::{open_api_lockfile, replace_privileged_config, ApiLockGuard};

use crate::init::{access_conf, acl_config, acl_config_lock};

pub fn split_acl_path(path: &str) -> Vec<&str> {
    let items = path.split('/');

    let mut components = vec![];

    for name in items {
        if name.is_empty() {
            continue;
        }
        components.push(name);
    }

    components
}

/// Tree representing a parsed acl.cfg
#[derive(Default)]
pub struct AclTree {
    /// Root node of the tree.
    ///
    /// The rest of the tree is available via [find_node()](AclTree::find_node()) or an
    /// [`AclTreeNode`]'s [children](AclTreeNode::children) member.
    pub root: AclTreeNode,
}

/// Node representing ACLs for a certain ACL path.
#[derive(Default)]
pub struct AclTreeNode {
    /// [User](pbs_api_types::User) or
    /// [Token](pbs_api_types::ApiToken) ACLs for this node.
    pub users: HashMap<Authid, HashMap<String, bool>>,
    /// `Group` ACLs for this node (not yet implemented)
    pub groups: HashMap<String, HashMap<String, bool>>,
    /// `AclTreeNodes` representing ACL paths directly below the current one.
    pub children: BTreeMap<String, AclTreeNode>,
}

impl AclTreeNode {
    /// Creates a new, empty AclTreeNode.
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            groups: HashMap::new(),
            children: BTreeMap::new(),
        }
    }

    /// Returns applicable role and their propagation status for a given [Authid].
    ///
    /// If the `Authid` is a [User](pbs_api_types::User) that has no specific `Roles` configured on
    /// this node, applicable `Group` roles will be returned instead.
    ///
    /// If `leaf` is `false`, only those roles where the propagate flag in the ACL is set to `true`
    /// are returned. Otherwise, all roles will be returned.
    pub fn extract_roles(&self, auth_id: &Authid, leaf: bool) -> HashMap<String, bool> {
        let user_roles = self.extract_user_roles(auth_id, leaf);
        if !user_roles.is_empty() || auth_id.is_token() {
            // user privs always override group privs
            return user_roles;
        };

        self.extract_group_roles(auth_id.user(), leaf)
    }

    fn extract_user_roles(&self, auth_id: &Authid, leaf: bool) -> HashMap<String, bool> {
        let mut map = HashMap::new();

        let roles = match self.users.get(auth_id) {
            Some(m) => m,
            None => return map,
        };

        for (role, propagate) in roles {
            if *propagate || leaf {
                if access_conf().role_no_access() == Some(role) {
                    // return a map with a single role 'NoAccess'
                    let mut map = HashMap::new();
                    map.insert(role.to_string(), false);
                    return map;
                }
                map.insert(role.to_string(), *propagate);
            }
        }

        map
    }

    fn extract_group_roles(&self, _user: &Userid, leaf: bool) -> HashMap<String, bool> {
        let mut map = HashMap::new();

        for roles in self.groups.values() {
            let is_member = false; // fixme: check if user is member of the group
            if !is_member {
                continue;
            }

            for (role, propagate) in roles {
                if *propagate || leaf {
                    if access_conf().role_no_access() == Some(role) {
                        // return a map with a single role 'NoAccess'
                        let mut map = HashMap::new();
                        map.insert(role.to_string(), false);
                        return map;
                    }
                    map.insert(role.to_string(), *propagate);
                }
            }
        }

        map
    }

    fn delete_group_role(&mut self, group: &str, role: &str) {
        let roles = match self.groups.get_mut(group) {
            Some(r) => r,
            None => return,
        };
        roles.remove(role);
    }

    fn delete_user_role(&mut self, auth_id: &Authid, role: &str) {
        let roles = match self.users.get_mut(auth_id) {
            Some(r) => r,
            None => return,
        };
        roles.remove(role);
    }

    fn delete_authid(&mut self, auth_id: &Authid) {
        for node in self.children.values_mut() {
            node.delete_authid(auth_id);
        }
        self.users.remove(auth_id);
    }

    fn insert_group_role(&mut self, group: String, role: String, propagate: bool) {
        let map = self.groups.entry(group).or_default();
        if let Some(no_access) = access_conf().role_no_access() {
            if role == no_access {
                map.clear();
            } else {
                map.remove(no_access);
            }
        }

        map.insert(role, propagate);
    }

    fn insert_user_role(&mut self, auth_id: Authid, role: String, propagate: bool) {
        let map = self.users.entry(auth_id).or_default();
        if let Some(no_access) = access_conf().role_no_access() {
            if role == no_access {
                map.clear();
            } else {
                map.remove(no_access);
            }
        }

        map.insert(role, propagate);
    }

    fn get_child_paths(
        &self,
        path: String,
        auth_id: &Authid,
        paths: &mut Vec<String>,
    ) -> Result<(), Error> {
        for (sub_comp, child_node) in &self.children {
            let roles = child_node.extract_roles(auth_id, true);
            let child_path = format!("{path}/{sub_comp}");
            if !roles.is_empty() {
                paths.push(child_path.clone());
            }
            child_node.get_child_paths(child_path, auth_id, paths)?;
        }
        Ok(())
    }
}

impl AclTree {
    /// Create a new, empty ACL tree with a single, empty root [node](AclTreeNode)
    pub fn new() -> Self {
        Self {
            root: AclTreeNode::new(),
        }
    }

    /// Iterates over the tree looking for a node matching `path`.
    pub fn find_node(&mut self, path: &str) -> Option<&mut AclTreeNode> {
        let path = split_acl_path(path);
        self.get_node_mut(&path)
    }

    fn get_node(&self, path: &[&str]) -> Option<&AclTreeNode> {
        let mut node = &self.root;
        for outer in path {
            for comp in outer.split('/') {
                node = match node.children.get(comp) {
                    Some(n) => n,
                    None => return None,
                };
            }
        }
        Some(node)
    }

    fn get_node_mut(&mut self, path: &[&str]) -> Option<&mut AclTreeNode> {
        let mut node = &mut self.root;
        for outer in path {
            for comp in outer.split('/') {
                node = match node.children.get_mut(comp) {
                    Some(n) => n,
                    None => return None,
                };
            }
        }
        Some(node)
    }

    fn get_or_insert_node(&mut self, path: &[&str]) -> &mut AclTreeNode {
        let mut node = &mut self.root;
        for outer in path {
            for comp in outer.split('/') {
                node = node.children.entry(String::from(comp)).or_default();
            }
        }
        node
    }

    /// Deletes the specified `role` from the `group`'s ACL on `path`.
    ///
    /// Never fails, even if the `path` has no ACLs configured, or the `group`/`role` combination
    /// does not exist on `path`.
    pub fn delete_group_role(&mut self, path: &str, group: &str, role: &str) {
        let path = split_acl_path(path);
        let node = match self.get_node_mut(&path) {
            Some(n) => n,
            None => return,
        };
        node.delete_group_role(group, role);
    }

    /// Deletes the specified `role` from the `user`'s ACL on `path`.
    ///
    /// Never fails, even if the `path` has no ACLs configured, or the `user`/`role` combination
    /// does not exist on `path`.
    pub fn delete_user_role(&mut self, path: &str, auth_id: &Authid, role: &str) {
        let path = split_acl_path(path);
        let node = match self.get_node_mut(&path) {
            Some(n) => n,
            None => return,
        };
        node.delete_user_role(auth_id, role);
    }

    /// Deletes the [`AclTreeNode`] at the specified patth
    ///
    /// Never fails, deletes a node iff the specified path exists.
    pub fn delete_node(&mut self, path: &str) {
        let mut path = split_acl_path(path);
        let last = path.pop();
        let parent = match self.get_node_mut(&path) {
            Some(n) => n,
            None => return,
        };
        if let Some(name) = last {
            parent.children.remove(name);
        }
    }

    /// Deletes a user or token from the ACL-tree
    ///
    /// Traverses the tree in-order and removes the given user/token by their Authid
    /// from every node in the tree.
    pub fn delete_authid(&mut self, auth_id: &Authid) {
        self.root.delete_authid(auth_id);
    }

    /// Inserts the specified `role` into the `group` ACL on `path`.
    ///
    /// The [`AclTreeNode`] representing `path` will be created and inserted into the tree if
    /// necessary.
    pub fn insert_group_role(&mut self, path: &str, group: &str, role: &str, propagate: bool) {
        let path = split_acl_path(path);
        let node = self.get_or_insert_node(&path);
        node.insert_group_role(group.to_string(), role.to_string(), propagate);
    }

    /// Inserts the specified `role` into the `user` ACL on `path`.
    ///
    /// The [`AclTreeNode`] representing `path` will be created and inserted into the tree if
    /// necessary.
    pub fn insert_user_role(&mut self, path: &str, auth_id: &Authid, role: &str, propagate: bool) {
        let path = split_acl_path(path);
        let node = self.get_or_insert_node(&path);
        node.insert_user_role(auth_id.to_owned(), role.to_string(), propagate);
    }

    fn write_node_config(node: &AclTreeNode, path: &str, w: &mut dyn Write) -> Result<(), Error> {
        let mut role_ug_map0: HashMap<_, BTreeSet<_>> = HashMap::new();
        let mut role_ug_map1: HashMap<_, BTreeSet<_>> = HashMap::new();

        for (auth_id, roles) in &node.users {
            // no need to save, because root is always 'Administrator'
            if !auth_id.is_token() && auth_id.user() == "root@pam" {
                continue;
            }
            for (role, propagate) in roles {
                let role = role.as_str();
                let auth_id = auth_id.to_string();
                if *propagate {
                    role_ug_map1.entry(role).or_default().insert(auth_id);
                } else {
                    role_ug_map0.entry(role).or_default().insert(auth_id);
                }
            }
        }

        for (group, roles) in &node.groups {
            for (role, propagate) in roles {
                let group = format!("@{}", group);
                if *propagate {
                    role_ug_map1.entry(role).or_default().insert(group);
                } else {
                    role_ug_map0.entry(role).or_default().insert(group);
                }
            }
        }

        fn group_by_property_list(
            item_property_map: &HashMap<&str, BTreeSet<String>>,
        ) -> BTreeMap<String, BTreeSet<String>> {
            let mut result_map: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
            for (item, property_map) in item_property_map {
                let item_list = property_map.iter().fold(String::new(), |mut acc, v| {
                    if !acc.is_empty() {
                        acc.push(',');
                    }
                    acc.push_str(v);
                    acc
                });
                result_map
                    .entry(item_list)
                    .or_default()
                    .insert(item.to_string());
            }
            result_map
        }

        let uglist_role_map0 = group_by_property_list(&role_ug_map0);
        let uglist_role_map1 = group_by_property_list(&role_ug_map1);

        fn role_list(roles: &BTreeSet<String>) -> String {
            if let Some(no_access) = access_conf().role_no_access() {
                if roles.contains(no_access) {
                    return String::from(no_access);
                }
            }

            roles.iter().fold(String::new(), |mut acc, v| {
                if !acc.is_empty() {
                    acc.push(',');
                }
                acc.push_str(v);
                acc
            })
        }

        for (uglist, roles) in &uglist_role_map0 {
            let role_list = role_list(roles);
            writeln!(
                w,
                "acl:0:{}:{}:{}",
                if path.is_empty() { "/" } else { path },
                uglist,
                role_list
            )?;
        }

        for (uglist, roles) in &uglist_role_map1 {
            let role_list = role_list(roles);
            writeln!(
                w,
                "acl:1:{}:{}:{}",
                if path.is_empty() { "/" } else { path },
                uglist,
                role_list
            )?;
        }

        for (name, child) in node.children.iter() {
            let child_path = format!("{}/{}", path, name);
            Self::write_node_config(child, &child_path, w)?;
        }

        Ok(())
    }

    fn write_config(&self, w: &mut dyn Write) -> Result<(), Error> {
        Self::write_node_config(&self.root, "", w)
    }

    fn parse_acl_line(&mut self, line: &str) -> Result<(), Error> {
        let items: Vec<&str> = line.split(':').collect();

        if items.len() != 5 {
            bail!("wrong number of items.");
        }

        if items[0] != "acl" {
            bail!("line does not start with 'acl'.");
        }

        let propagate = if items[1] == "0" {
            false
        } else if items[1] == "1" {
            true
        } else {
            bail!("expected '0' or '1' for propagate flag.");
        };

        let path_str = items[2];
        let path = split_acl_path(path_str);
        let node = self.get_or_insert_node(&path);

        let uglist: Vec<&str> = items[3].split(',').map(|v| v.trim()).collect();

        let rolelist: Vec<&str> = items[4].split(',').map(|v| v.trim()).collect();

        for user_or_group in &uglist {
            for role in &rolelist {
                if !access_conf().roles().contains_key(role) {
                    bail!("unknown role '{}'", role);
                }
                if let Some(group) = user_or_group.strip_prefix('@') {
                    node.insert_group_role(group.to_string(), role.to_string(), propagate);
                } else {
                    node.insert_user_role(user_or_group.parse()?, role.to_string(), propagate);
                }
            }
        }

        Ok(())
    }

    fn load(filename: &Path) -> Result<(Self, [u8; 32]), Error> {
        let mut tree = Self::new();

        let raw = match std::fs::read_to_string(filename) {
            Ok(v) => v,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    String::new()
                } else {
                    bail!("unable to read acl config {:?} - {}", filename, err);
                }
            }
        };

        let digest = openssl::sha::sha256(raw.as_bytes());

        for (linenr, line) in raw.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Err(err) = tree.parse_acl_line(line) {
                bail!(
                    "unable to parse acl config {:?}, line {} - {}",
                    filename,
                    linenr + 1,
                    err
                );
            }
        }

        Ok((tree, digest))
    }

    /// This is used for testing
    pub fn from_raw(raw: &str) -> Result<Self, Error> {
        let mut tree = Self::new();
        for (linenr, line) in raw.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Err(err) = tree.parse_acl_line(line) {
                bail!(
                    "unable to parse acl config data, line {} - {}",
                    linenr + 1,
                    err
                );
            }
        }
        Ok(tree)
    }

    /// Returns a map of role name and propagation status for a given `auth_id` and `path`.
    ///
    /// This will collect role mappings according to the following algorithm:
    /// - iterate over all intermediate nodes along `path` and collect roles with `propagate` set
    /// - get all (propagating and non-propagating) roles for last component of path
    /// - more specific role maps replace less specific role maps
    /// -- user/token is more specific than group at each level
    /// -- roles lower in the tree are more specific than those higher up along the path
    pub fn roles(&self, auth_id: &Authid, path: &[&str]) -> HashMap<String, bool> {
        let mut node = &self.root;
        let mut role_map = node.extract_roles(auth_id, path.is_empty());

        let mut comp_iter = path.iter().peekable();

        while let Some(comp) = comp_iter.next() {
            let last_comp = comp_iter.peek().is_none();

            let mut sub_comp_iter = comp.split('/').peekable();

            while let Some(sub_comp) = sub_comp_iter.next() {
                let last_sub_comp = last_comp && sub_comp_iter.peek().is_none();

                node = match node.children.get(sub_comp) {
                    Some(n) => n,
                    None => return role_map, // path not found
                };

                let new_map = node.extract_roles(auth_id, last_sub_comp);
                if !new_map.is_empty() {
                    // overwrite previous mappings
                    role_map = new_map;
                }
            }
        }

        role_map
    }

    pub fn get_child_paths(&self, auth_id: &Authid, path: &[&str]) -> Result<Vec<String>, Error> {
        let mut res = Vec::new();

        if let Some(node) = self.get_node(path) {
            let path = path.join("/");
            node.get_child_paths(path, auth_id, &mut res)?;
        }

        Ok(res)
    }
}

/// Get exclusive lock
pub fn lock_config() -> Result<ApiLockGuard, Error> {
    open_api_lockfile(acl_config_lock(), None, true)
}

/// Reads the [`AclTree`] from the [default path](ACL_CFG_FILENAME).
pub fn config() -> Result<(AclTree, [u8; 32]), Error> {
    let path = acl_config();
    AclTree::load(&path)
}

/// Returns a cached [`AclTree`] or fresh copy read directly from the [default
/// path](ACL_CFG_FILENAME)
///
/// Since the AclTree is used for every API request's permission check, this caching mechanism
/// allows to skip reading and parsing the file again if it is unchanged.
pub fn cached_config() -> Result<Arc<AclTree>, Error> {
    struct ConfigCache {
        data: Option<Arc<AclTree>>,
        last_mtime: i64,
        last_mtime_nsec: i64,
    }

    static CACHED_CONFIG: OnceLock<RwLock<ConfigCache>> = OnceLock::new();
    let cached_conf = CACHED_CONFIG.get_or_init(|| {
        RwLock::new(ConfigCache {
            data: None,
            last_mtime: 0,
            last_mtime_nsec: 0,
        })
    });

    let conf = acl_config();
    let stat = match nix::sys::stat::stat(&conf) {
        Ok(stat) => Some(stat),
        Err(nix::errno::Errno::ENOENT) => None,
        Err(err) => bail!("unable to stat '{}' - {err}", conf.display()),
    };

    {
        // limit scope
        let cache = cached_conf.read().unwrap();
        if let Some(ref config) = cache.data {
            if let Some(stat) = stat {
                if stat.st_mtime == cache.last_mtime && stat.st_mtime_nsec == cache.last_mtime_nsec
                {
                    return Ok(config.clone());
                }
            } else if cache.last_mtime == 0 && cache.last_mtime_nsec == 0 {
                return Ok(config.clone());
            }
        }
    }

    let (config, _digest) = config()?;
    let config = Arc::new(config);

    let mut cache = cached_conf.write().unwrap();
    if let Some(stat) = stat {
        cache.last_mtime = stat.st_mtime;
        cache.last_mtime_nsec = stat.st_mtime_nsec;
    }
    cache.data = Some(config.clone());

    Ok(config)
}

/// Saves an [`AclTree`] to the [default path](ACL_CFG_FILENAME), ensuring proper ownership and
/// file permissions.
pub fn save_config(acl: &AclTree) -> Result<(), Error> {
    let mut raw: Vec<u8> = Vec::new();
    acl.write_config(&mut raw)?;

    let conf = acl_config();
    replace_privileged_config(conf, &raw)?;

    // increase cache generation so we reload it next time we access it
    access_conf().increment_cache_generation()?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, sync::OnceLock};

    use crate::init::{init_access_config, AccessControlConfig};

    use super::AclTree;
    use anyhow::Error;

    use proxmox_auth_api::types::Authid;

    #[derive(Debug)]
    struct TestAcmConfig<'a> {
        roles: HashMap<&'a str, u64>,
    }

    impl AccessControlConfig for TestAcmConfig<'_> {
        fn roles(&self) -> &HashMap<&str, u64> {
            &self.roles
        }

        fn privileges(&self) -> &HashMap<&str, u64> {
            unreachable!("acl tests don't need privileges")
        }

        fn role_no_access(&self) -> Option<&'static str> {
            Some("NoAccess")
        }

        fn role_admin(&self) -> Option<&'static str> {
            Some("Admin")
        }
    }

    fn setup_acl_tree_config() {
        static ACL_CONFIG: OnceLock<TestAcmConfig> = OnceLock::new();
        let config = ACL_CONFIG.get_or_init(|| {
            let mut roles = HashMap::new();
            roles.insert("NoAccess", 0);
            roles.insert("Admin", u64::MAX);
            roles.insert("DatastoreBackup", 4);
            roles.insert("DatastoreReader", 8);

            let config = TestAcmConfig { roles };
            config
        });

        // ignore errors here, we don't care if it's initialized already
        let _ = init_access_config(config);
    }

    fn check_roles(tree: &AclTree, auth_id: &Authid, path: &str, expected_roles: &str) {
        let path_vec = super::split_acl_path(path);
        let mut roles = tree
            .roles(auth_id, &path_vec)
            .keys()
            .cloned()
            .collect::<Vec<String>>();
        roles.sort();
        let roles = roles.join(",");

        assert_eq!(
            roles, expected_roles,
            "\nat check_roles for '{}' on '{}'",
            auth_id, path
        );
    }

    #[test]
    fn test_acl_line_compression() {
        setup_acl_tree_config();

        let tree = AclTree::from_raw(
            "\
            acl:0:/store/store2:user1@pbs:Admin\n\
            acl:0:/store/store2:user2@pbs:Admin\n\
            acl:0:/store/store2:user1@pbs:DatastoreBackup\n\
            acl:0:/store/store2:user2@pbs:DatastoreBackup\n\
            ",
        )
        .expect("failed to parse acl tree");

        let mut raw: Vec<u8> = Vec::new();
        tree.write_config(&mut raw)
            .expect("failed to write acl tree");
        let raw = std::str::from_utf8(&raw).expect("acl tree is not valid utf8");

        assert_eq!(
            raw,
            "acl:0:/store/store2:user1@pbs,user2@pbs:Admin,DatastoreBackup\n"
        );
    }

    #[test]
    fn test_roles_1() -> Result<(), Error> {
        setup_acl_tree_config();

        let tree = AclTree::from_raw(
            "\
            acl:1:/storage:user1@pbs:Admin\n\
            acl:1:/storage/store1:user1@pbs:DatastoreBackup\n\
            acl:1:/storage/store2:user2@pbs:DatastoreBackup\n\
            ",
        )?;
        let user1: Authid = "user1@pbs".parse()?;
        check_roles(&tree, &user1, "/", "");
        check_roles(&tree, &user1, "/storage", "Admin");
        check_roles(&tree, &user1, "/storage/store1", "DatastoreBackup");
        check_roles(&tree, &user1, "/storage/store2", "Admin");

        let user2: Authid = "user2@pbs".parse()?;
        check_roles(&tree, &user2, "/", "");
        check_roles(&tree, &user2, "/storage", "");
        check_roles(&tree, &user2, "/storage/store1", "");
        check_roles(&tree, &user2, "/storage/store2", "DatastoreBackup");

        Ok(())
    }

    #[test]
    fn test_role_no_access() -> Result<(), Error> {
        setup_acl_tree_config();

        let tree = AclTree::from_raw(
            "\
            acl:1:/:user1@pbs:Admin\n\
            acl:1:/storage:user1@pbs:NoAccess\n\
            acl:1:/storage/store1:user1@pbs:DatastoreBackup\n\
            ",
        )?;
        let user1: Authid = "user1@pbs".parse()?;
        check_roles(&tree, &user1, "/", "Admin");
        check_roles(&tree, &user1, "/storage", "NoAccess");
        check_roles(&tree, &user1, "/storage/store1", "DatastoreBackup");
        check_roles(&tree, &user1, "/storage/store2", "NoAccess");
        check_roles(&tree, &user1, "/system", "Admin");

        let tree = AclTree::from_raw(
            "\
            acl:1:/:user1@pbs:Admin\n\
            acl:0:/storage:user1@pbs:NoAccess\n\
            acl:1:/storage/store1:user1@pbs:DatastoreBackup\n\
            ",
        )?;
        check_roles(&tree, &user1, "/", "Admin");
        check_roles(&tree, &user1, "/storage", "NoAccess");
        check_roles(&tree, &user1, "/storage/store1", "DatastoreBackup");
        check_roles(&tree, &user1, "/storage/store2", "Admin");
        check_roles(&tree, &user1, "/system", "Admin");

        Ok(())
    }

    #[test]
    fn test_role_add_delete() -> Result<(), Error> {
        setup_acl_tree_config();

        let mut tree = AclTree::new();

        let user1: Authid = "user1@pbs".parse()?;

        tree.insert_user_role("/", &user1, "Admin", true);
        tree.insert_user_role("/", &user1, "Audit", true);

        check_roles(&tree, &user1, "/", "Admin,Audit");

        tree.insert_user_role("/", &user1, "NoAccess", true);
        check_roles(&tree, &user1, "/", "NoAccess");

        let mut raw: Vec<u8> = Vec::new();
        tree.write_config(&mut raw)?;
        let raw = std::str::from_utf8(&raw)?;

        assert_eq!(raw, "acl:1:/:user1@pbs:NoAccess\n");

        Ok(())
    }

    #[test]
    fn test_no_access_overwrite() -> Result<(), Error> {
        setup_acl_tree_config();

        let mut tree = AclTree::new();

        let user1: Authid = "user1@pbs".parse()?;

        tree.insert_user_role("/storage", &user1, "NoAccess", true);

        check_roles(&tree, &user1, "/storage", "NoAccess");

        tree.insert_user_role("/storage", &user1, "Admin", true);
        tree.insert_user_role("/storage", &user1, "Audit", true);

        check_roles(&tree, &user1, "/storage", "Admin,Audit");

        tree.insert_user_role("/storage", &user1, "NoAccess", true);

        check_roles(&tree, &user1, "/storage", "NoAccess");

        Ok(())
    }

    #[test]
    fn test_get_child_paths() -> Result<(), Error> {
        setup_acl_tree_config();

        let tree = AclTree::from_raw(
            "\
            acl:0:/store/store2:user1@pbs:Admin\n\
            acl:1:/store/store2/store31/store4/store6:user2@pbs:DatastoreReader\n\
            acl:0:/store/store2/store3:user1@pbs:Admin\n\
            ",
        )
        .expect("failed to parse acl tree");

        let user1: Authid = "user1@pbs".parse()?;
        let user2: Authid = "user2@pbs".parse()?;

        // user1 has admin on "/store/store2/store3" -> return paths
        let paths = tree.get_child_paths(&user1, &["store"])?;
        assert!(
            paths.len() == 2
                && paths.contains(&"store/store2".to_string())
                && paths.contains(&"store/store2/store3".to_string())
        );

        // user2 has no privileges under "/store/store2/store3" --> return empty
        assert!(tree
            .get_child_paths(&user2, &["store", "store2", "store3"],)?
            .is_empty());

        // user2 has DatastoreReader privileges under "/store/store2/store31" --> return paths
        let paths = tree.get_child_paths(&user2, &["store/store2/store31"])?;
        assert!(
            paths.len() == 1 && paths.contains(&"store/store2/store31/store4/store6".to_string())
        );

        // user2 has no privileges under "/store/store2/foo/bar/baz"
        assert!(tree
            .get_child_paths(&user2, &["store", "store2", "foo/bar/baz"])?
            .is_empty());

        // user2 has DatastoreReader privileges on "/store/store2/store31/store4/store6", but not
        // on any child paths --> return empty
        assert!(tree
            .get_child_paths(&user2, &["store/store2/store31/store4/store6"],)?
            .is_empty());

        Ok(())
    }

    #[test]
    fn test_delete_node() -> Result<(), Error> {
        setup_acl_tree_config();

        let mut tree = AclTree::new();

        let user1: Authid = "user1@pbs".parse()?;

        tree.insert_user_role("/storage", &user1, "NoAccess", true);
        tree.insert_user_role("/storage/a", &user1, "NoAccess", true);
        tree.insert_user_role("/storage/b", &user1, "NoAccess", true);
        tree.insert_user_role("/storage/b/a", &user1, "NoAccess", true);
        tree.insert_user_role("/storage/b/b", &user1, "NoAccess", true);
        tree.insert_user_role("/datastore/c", &user1, "NoAccess", true);
        tree.insert_user_role("/datastore/d", &user1, "NoAccess", true);

        assert!(tree.find_node("/storage/b/a").is_some());
        tree.delete_node("/storage/b/a");
        assert!(tree.find_node("/storage/b/a").is_none());

        assert!(tree.find_node("/storage/b/b").is_some());
        assert!(tree.find_node("/storage/b").is_some());
        tree.delete_node("/storage/b");
        assert!(tree.find_node("/storage/b/b").is_none());
        assert!(tree.find_node("/storage/b").is_none());

        assert!(tree.find_node("/storage").is_some());
        assert!(tree.find_node("/storage/a").is_some());
        tree.delete_node("/storage");
        assert!(tree.find_node("/storage").is_none());
        assert!(tree.find_node("/storage/a").is_none());

        assert!(tree.find_node("/datastore/c").is_some());
        tree.delete_node("/datastore/c");
        assert!(tree.find_node("/datastore/c").is_none());

        assert!(tree.find_node("/datastore/d").is_some());
        tree.delete_node("/datastore/d");
        assert!(tree.find_node("/datastore/d").is_none());

        // '/' should not be deletable
        assert!(tree.find_node("/").is_some());
        tree.delete_node("/");
        assert!(tree.find_node("/").is_some());

        Ok(())
    }

    #[test]
    fn test_delete_authid() -> Result<(), Error> {
        setup_acl_tree_config();

        let mut tree = AclTree::new();

        let user1: Authid = "user1@pbs".parse()?;
        let user2: Authid = "user2@pbs".parse()?;

        let user1_paths = vec![
            "/",
            "/storage",
            "/storage/a",
            "/storage/a/b",
            "/storage/b",
            "/storage/b/a",
            "/storage/b/b",
            "/storage/a/a",
        ];
        let user2_paths = vec!["/", "/storage", "/storage/a/b", "/storage/a/a"];

        for path in &user1_paths {
            tree.insert_user_role(path, &user1, "NoAccess", true);
        }
        for path in &user2_paths {
            tree.insert_user_role(path, &user2, "NoAccess", true);
        }

        tree.delete_authid(&user1);

        for path in &user1_paths {
            let node = tree.find_node(path);
            assert!(node.is_some());
            if let Some(node) = node {
                assert!(!node.users.contains_key(&user1));
            }
        }
        for path in &user2_paths {
            let node = tree.find_node(path);
            assert!(node.is_some());
            if let Some(node) = node {
                assert!(node.users.contains_key(&user2));
            }
        }

        tree.delete_authid(&user2);

        for path in &user2_paths {
            let node = tree.find_node(path);
            assert!(node.is_some());
            if let Some(node) = node {
                assert!(!node.users.contains_key(&user2));
            }
        }

        Ok(())
    }
}
