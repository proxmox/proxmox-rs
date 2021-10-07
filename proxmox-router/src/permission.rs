//! Declarative permission system
//!
//! A declarative way to define API access permissions.

use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

/// Access permission
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub enum Permission {
    /// Allow Superuser
    Superuser,
    /// Allow the whole World, no authentication required
    World,
    /// Allow any authenticated user
    Anybody,
    /// Allow access for the specified user
    User(&'static str),
    /// Allow access if specified param matches logged in user
    UserParam(&'static str),
    /// Allow access for the specified group of users
    Group(&'static str),
    /// Use a parameter value as userid to run sub-permission tests.
    WithParam(&'static str, &'static Permission),
    /// Check privilege/role on the specified path. The boolean
    /// attribute specifies if you want to allow partial matches (u64
    /// interpreted as bitmask).
    Privilege(&'static [&'static str], u64, bool),
    /// Allow access if all sub-permissions match
    And(&'static [&'static Permission]),
    /// Allow access if any sub-permissions match
    Or(&'static [&'static Permission]),
}

impl fmt::Debug for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::Superuser => f.write_str("Superuser"),
            Permission::World => f.write_str("World"),
            Permission::Anybody => f.write_str("Anybody"),
            Permission::User(ref userid) => write!(f, "User({})", userid),
            Permission::UserParam(param_name) => write!(f, "UserParam({})", param_name),
            Permission::Group(ref group) => write!(f, "Group({})", group),
            Permission::WithParam(param_name, subtest) => {
                write!(f, "WithParam({}, {:?})", param_name, subtest)
            }
            Permission::Privilege(path, privs, partial) => {
                write!(f, "Privilege({:?}, {:0b}, {})", path, privs, partial)
            }
            Permission::And(list) => {
                f.write_str("And(\n")?;
                for subtest in list.iter() {
                    writeln!(f, "  {:?}", subtest)?;
                }
                f.write_str(")\n")
            }
            Permission::Or(list) => {
                f.write_str("Or(\n")?;
                for subtest in list.iter() {
                    writeln!(f, "  {:?}", subtest)?;
                }
                f.write_str(")\n")
            }
        }
    }
}

/// Trait to query user information (used by check_api_permission)
pub trait UserInformation {
    fn is_superuser(&self, userid: &str) -> bool;
    fn is_group_member(&self, userid: &str, group: &str) -> bool;
    fn lookup_privs(&self, userid: &str, path: &[&str]) -> u64;
}

impl <T: UserInformation> UserInformation for std::sync::Arc<T> {
    fn is_superuser(&self, userid: &str) -> bool {
        self.deref().is_superuser(userid)
    }
    fn is_group_member(&self, userid: &str, group: &str) -> bool {
        self.deref().is_group_member(userid, group)
    }
    fn lookup_privs(&self, userid: &str, path: &[&str]) -> u64 {
        self.deref().lookup_privs(userid, path)
    }
}

/// Example implementation to check access permissions
///
/// This implementation supports URI variables in Privilege path
/// components, i.e. '{storage}'. We replace this with actual
/// parameter values before calling lookup_privs().
pub fn check_api_permission(
    perm: &Permission,
    userid: Option<&str>,
    param: &HashMap<String, String>,
    info: &dyn UserInformation,
) -> bool {
    if let Some(ref userid) = userid {
        if info.is_superuser(userid) {
            return true;
        }
    }

    check_api_permission_tail(perm, userid, param, info)
}

// some of them are deeply nested
#[allow(clippy::needless_return)]
fn check_api_permission_tail(
    perm: &Permission,
    userid: Option<&str>,
    param: &HashMap<String, String>,
    info: &dyn UserInformation,
) -> bool {
    match perm {
        Permission::World => return true,
        Permission::Anybody => {
            return userid.is_some();
        }
        Permission::Superuser => match userid {
            None => return false,
            Some(ref userid) => return info.is_superuser(userid),
        },
        Permission::User(expected_userid) => match userid {
            None => return false,
            Some(ref userid) => return userid == expected_userid,
        },
        Permission::UserParam(param_name) => match (userid, param.get(&param_name.to_string())) {
            (None, _) => return false,
            (_, None) => return false,
            (Some(ref userid), Some(ref expected)) => return userid == expected,
        },
        Permission::Group(expected_group) => match userid {
            None => return false,
            Some(ref userid) => return info.is_group_member(userid, expected_group),
        },
        Permission::WithParam(param_name, subtest) => {
            return check_api_permission(
                subtest,
                param.get(*param_name).map(|v| v.as_str()),
                param,
                info,
            );
        }
        Permission::Privilege(path, expected_privs, partial) => {
            // replace uri vars
            let mut new_path: Vec<&str> = Vec::new();
            for comp in path.iter() {
                if comp.starts_with('{') && comp.ends_with('}') {
                    let param_name = unsafe { comp.get_unchecked(1..comp.len() - 1) };
                    match param.get(param_name) {
                        None => return false,
                        Some(value) => {
                            new_path.push(value);
                        }
                    }
                } else {
                    new_path.push(comp);
                }
            }
            match userid {
                None => return false,
                Some(userid) => {
                    let privs = info.lookup_privs(userid, &new_path);
                    if privs == 0 {
                        return false;
                    };
                    if *partial {
                        return (expected_privs & privs) != 0;
                    } else {
                        return (*expected_privs & privs) == *expected_privs;
                    }
                }
            }
        }
        Permission::And(list) => {
            for subtest in list.iter() {
                if !check_api_permission_tail(subtest, userid, param, info) {
                    return false;
                }
            }

            return true;
        }
        Permission::Or(list) => {
            for subtest in list.iter() {
                if check_api_permission_tail(subtest, userid, param, info) {
                    return true;
                }
            }

            return false;
        }
    }
}

#[cfg(test)]
mod test {
    use serde_json::{json, Value};

    use crate::permission::*;

    struct MockedUserInfo {
        privs: Value,
        groups: Value,
    }

    impl UserInformation for MockedUserInfo {
        fn is_superuser(&self, userid: &str) -> bool {
            userid == "root"
        }

        fn is_group_member(&self, userid: &str, group: &str) -> bool {
            if let Some(groups) = self.groups[userid].as_array() {
                return groups.contains(&Value::from(group));
            }

            return false;
        }

        fn lookup_privs(&self, userid: &str, path: &[&str]) -> u64 {
            let path = format!("/{}", path.join("/"));
            if let Some(users) = self.privs.get(path) {
                if let Some(privilege) = users.get(userid) {
                    return privilege.as_u64().unwrap();
                }
            }

            return 0;
        }
    }

    #[test]
    fn test_privileges() {
        let userinfo = MockedUserInfo {
            privs: json!({
                "/": {
                    "user1": 0b10,
                },
                "/datastore": {
                    "user1": 0b00,
                    "user2": 0b01,
                },
                "/datastore/foo": {
                    "user1": 0b01,
                },
            }),
            groups: json!({
                "user1": [
                    "group1",
                ],
                "user2": [
                    "group2",
                ],
            }),
        };

        let mut param = HashMap::new();
        param.insert("user".to_string(), "user1".to_string());
        param.insert("datastore".to_string(), "foo".to_string());

        let test_check = |perm: &Permission, userid: Option<&str>, should_succeed: bool| {
            println!("{:?} on {:?}: {}", userid, perm, should_succeed);
            assert_eq!(
                check_api_permission(perm, userid, &param, &userinfo),
                should_succeed
            )
        };

        test_check(&Permission::Superuser, Some("root"), true);
        test_check(&Permission::Superuser, Some("user1"), false);
        test_check(&Permission::Superuser, None, false);

        test_check(&Permission::World, Some("root"), true);
        test_check(&Permission::World, Some("user1"), true);
        test_check(&Permission::World, None, true);

        test_check(&Permission::Anybody, Some("root"), true);
        test_check(&Permission::Anybody, Some("user1"), true);
        test_check(&Permission::Anybody, None, false);

        test_check(&Permission::User("user1"), Some("root"), true);
        test_check(&Permission::User("user1"), Some("user1"), true);
        test_check(&Permission::User("user1"), Some("user2"), false);
        test_check(&Permission::User("user1"), None, false);

        test_check(&Permission::Group("group1"), Some("root"), true);
        test_check(&Permission::Group("group1"), Some("user1"), true);
        test_check(&Permission::Group("group1"), Some("user2"), false);
        test_check(&Permission::Group("group1"), None, false);

        test_check(
            &Permission::WithParam("user", &Permission::User("root")),
            Some("root"),
            true,
        );
        test_check(
            &Permission::WithParam("user", &Permission::User("user1")),
            Some("user1"),
            true,
        );
        test_check(
            &Permission::WithParam("user", &Permission::User("user2")),
            Some("user2"),
            false,
        );
        test_check(
            &Permission::WithParam("user", &Permission::User("")),
            None,
            false,
        );

        test_check(
            &Permission::And(&[&Permission::User("user1"), &Permission::Group("group2")]),
            Some("root"),
            true,
        );
        test_check(
            &Permission::And(&[&Permission::User("user1"), &Permission::Group("group2")]),
            Some("user1"),
            false,
        );
        test_check(
            &Permission::And(&[&Permission::User("user1"), &Permission::Group("group1")]),
            Some("user1"),
            true,
        );
        test_check(
            &Permission::And(&[&Permission::User("user1"), &Permission::Group("group2")]),
            None,
            false,
        );

        test_check(
            &Permission::Or(&[&Permission::User("user1"), &Permission::Group("group2")]),
            Some("root"),
            true,
        );
        test_check(
            &Permission::Or(&[&Permission::User("user1"), &Permission::Group("group2")]),
            Some("user1"),
            true,
        );
        test_check(
            &Permission::Or(&[&Permission::User("user1"), &Permission::Group("group1")]),
            Some("user2"),
            false,
        );
        test_check(
            &Permission::Or(&[&Permission::User("user1"), &Permission::Group("group2")]),
            None,
            false,
        );

        test_check(&Permission::Privilege(&[], 0b11, true), Some("root"), true);
        test_check(&Permission::Privilege(&[], 0b11, true), Some("user1"), true);
        test_check(
            &Permission::Privilege(&[], 0b11, false),
            Some("user1"),
            false,
        );
        test_check(
            &Permission::Privilege(&[], 0b11, true),
            Some("user2"),
            false,
        );
        test_check(
            &Permission::Privilege(&[], 0b11, false),
            Some("user2"),
            false,
        );
        test_check(&Permission::Privilege(&[], 0b11, true), None, false);
        test_check(&Permission::Privilege(&[], 0b11, false), None, false);

        test_check(
            &Permission::Privilege(&["datastore"], 0b01, true),
            Some("user1"),
            false,
        );
        test_check(
            &Permission::Privilege(&["datastore"], 0b01, true),
            Some("user2"),
            true,
        );
        test_check(
            &Permission::Privilege(&["datastore"], 0b01, true),
            None,
            false,
        );

        test_check(
            &Permission::Privilege(&["datastore", "{datastore}"], 0b01, true),
            Some("user1"),
            true,
        );
        test_check(
            &Permission::Privilege(&["datastore", "{datastore}"], 0b01, true),
            Some("user2"),
            false,
        );
        test_check(
            &Permission::Privilege(&["datastore", "{datastore}"], 0b01, true),
            None,
            false,
        );
    }
}
