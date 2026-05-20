use std::path::PathBuf;

use anyhow::{bail, format_err, Error};

use proxmox_apt::repositories::{
    check_repositories, get_current_release_codename, standard_repositories,
    standard_repos_offered_for, DebianCodename,
};
use proxmox_apt::repositories::{APTRepositoryFileImpl, APTRepositoryImpl};
use proxmox_apt_api_types::{
    APTRepositoryFile, APTRepositoryHandle, APTRepositoryInfo, APTStandardRepository, HostProduct,
};

fn create_clean_directory(path: &PathBuf) -> Result<(), Error> {
    match std::fs::remove_dir_all(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (),
        Err(err) => bail!("unable to remove dir {path:?} - {err}"),
        Ok(_) => (),
    }
    std::fs::create_dir_all(path)
        .map_err(|err| format_err!("unable to create dir {path:?} - {err}"))
}

#[test]
fn test_parse_write() -> Result<(), Error> {
    test_parse_write_dir("sources.list.d")?;
    test_parse_write_dir("sources.list.d.expected")?; // check if it's idempotent

    Ok(())
}

fn test_parse_write_dir(read_dir: &str) -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let tmp_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR").to_string());
    let read_dir = test_dir.join(read_dir);
    let write_dir = tmp_dir.join("sources.list.d.actual");
    let expected_dir = test_dir.join("sources.list.d.expected");

    create_clean_directory(&write_dir)?;

    let mut files = vec![];
    let mut errors = vec![];

    for entry in std::fs::read_dir(read_dir)? {
        let path = entry?.path();

        match APTRepositoryFile::new(&path)? {
            Some(mut file) => match file.parse() {
                Ok(()) => files.push(file),
                Err(err) => errors.push(err),
            },
            None => bail!("unexpected None for '{:?}'", path),
        }
    }

    assert!(errors.is_empty());

    for file in files.iter_mut() {
        let path = match &file.path {
            Some(path) => path,
            None => continue,
        };
        let path = PathBuf::from(path);
        let new_path = write_dir.join(path.file_name().unwrap());
        file.path = Some(new_path.into_os_string().into_string().unwrap());
        file.digest = None;
        file.write()?;
    }

    let mut expected_count = 0;

    for entry in std::fs::read_dir(expected_dir)? {
        expected_count += 1;

        let expected_path = entry?.path();

        let actual_path = write_dir.join(expected_path.file_name().unwrap());

        let expected_contents = std::fs::read(&expected_path)
            .map_err(|err| format_err!("unable to read {expected_path:?} - {err}"))?;

        let actual_contents = std::fs::read(&actual_path)
            .map_err(|err| format_err!("unable to read {actual_path:?} - {err}"))?;

        assert_eq!(
            expected_contents, actual_contents,
            "Use\n\ndiff -u {expected_path:?} {actual_path:?}\n\nif you're not fluent in byte decimals"
        );
    }

    let actual_count = std::fs::read_dir(write_dir)?.count();

    assert_eq!(expected_count, actual_count);

    Ok(())
}

#[test]
fn test_digest() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let tmp_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR").to_string());
    let read_dir = test_dir.join("sources.list.d");
    let write_dir = tmp_dir.join("sources.list.d.digest");

    create_clean_directory(&write_dir)?;

    let path = read_dir.join("standard.list");

    let mut file = APTRepositoryFile::new(&path)?.unwrap();
    file.parse()?;

    let new_path = write_dir.join(path.file_name().unwrap());
    file.path = Some(new_path.clone().into_os_string().into_string().unwrap());

    let old_digest = file.digest.unwrap();

    // file does not exist yet...
    assert!(file.read_with_digest().is_err());
    assert!(file.write().is_err());

    // ...but it should work if there's no digest
    file.digest = None;
    file.write()?;

    // overwrite with old contents...
    std::fs::copy(path, new_path)?;

    // modify the repo
    let repo = file.repositories.first_mut().unwrap();
    repo.enabled = !repo.enabled;

    // ...then it should work
    file.digest = Some(old_digest);
    file.write()?;

    // expect a different digest, because the repo was modified
    let (_, new_digest) = file.read_with_digest()?;
    assert_ne!(old_digest, *new_digest);

    assert!(file.write().is_err());

    Ok(())
}

#[test]
fn test_empty_write() -> Result<(), Error> {
    let write_dir = PathBuf::from(
        std::option_env!("CARGO_TARGET_TMPDIR").expect("no test target dir set by cargo!"),
    )
    .join("tests")
    .join("sources.list.d.remove");
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");

    create_clean_directory(&write_dir)?;

    let path = read_dir.join("standard.list");

    let mut file = APTRepositoryFile::new(&path)?.unwrap();
    file.parse()?;

    let new_path = write_dir.join(path.file_name().unwrap());
    file.path = Some(new_path.into_os_string().into_string().unwrap());

    file.digest = None;

    file.write()?;

    assert!(file.exists());

    file.repositories.clear();

    file.write()?;

    assert!(!file.exists());

    Ok(())
}

#[test]
fn test_check_repositories() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");
    let apt_lists_dir: PathBuf = test_dir.join("lists");

    let absolute_suite_list = read_dir.join("absolute_suite.list");
    let mut file = APTRepositoryFile::new(absolute_suite_list)?.unwrap();
    file.parse()?;

    let infos = check_repositories(&[file], &DebianCodename::Bullseye, &apt_lists_dir);

    assert!(infos.is_empty());
    let pve_list = read_dir.join("pve.list");
    let mut file = APTRepositoryFile::new(&pve_list)?.unwrap();
    file.parse()?;

    let path_string = pve_list.into_os_string().into_string().unwrap();

    let origins = [
        "Debian", "Debian", "Proxmox", "Proxmox", "Proxmox", "Debian",
    ];

    let mut expected_infos = vec![];
    for (n, origin) in origins.into_iter().enumerate() {
        expected_infos.push(APTRepositoryInfo {
            path: path_string.clone(),
            index: n,
            property: None,
            kind: "origin".to_string(),
            message: origin.to_string(),
        });
    }
    expected_infos.sort();

    let mut infos = check_repositories(&[file], &DebianCodename::Bullseye, &apt_lists_dir);
    infos.sort();

    assert_eq!(infos, expected_infos);

    let bad_sources = read_dir.join("bad.sources");
    let mut file = APTRepositoryFile::new(&bad_sources)?.unwrap();
    file.parse()?;

    let path_string = bad_sources.into_os_string().into_string().unwrap();

    let mut expected_infos = vec![
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 0,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "suite 'sid' should not be used in production!".to_string(),
        },
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 1,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "old suite 'lenny' configured!".to_string(),
        },
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 2,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "old suite 'stretch' configured!".to_string(),
        },
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 3,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "use the name of the stable distribution instead of 'stable'!".to_string(),
        },
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 4,
            property: Some("Suites".to_string()),
            kind: "ignore-pre-upgrade-warning".to_string(),
            message: "suite 'bookworm' should not be used in production!".to_string(),
        },
        APTRepositoryInfo {
            path: path_string.clone(),
            index: 5,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "suite 'testing' should not be used in production!".to_string(),
        },
    ];
    for n in 0..=5 {
        expected_infos.push(APTRepositoryInfo {
            path: path_string.clone(),
            index: n,
            property: None,
            kind: "origin".to_string(),
            message: "Debian".to_string(),
        });
    }
    expected_infos.sort();

    let mut infos = check_repositories(&[file], &DebianCodename::Bullseye, &apt_lists_dir);
    infos.sort();

    assert_eq!(infos, expected_infos);

    let bad_security = read_dir.join("bad-security.list");
    let mut file = APTRepositoryFile::new(&bad_security)?.unwrap();
    file.parse()?;

    let path_string = bad_security.into_os_string().into_string().unwrap();

    let mut expected_infos = vec![];
    for n in 0..=1 {
        expected_infos.push(APTRepositoryInfo {
            path: path_string.clone(),
            index: n,
            property: Some("Suites".to_string()),
            kind: "warning".to_string(),
            message: "expected suite 'bullseye-security'".to_string(),
        });
    }
    for n in 0..=1 {
        expected_infos.push(APTRepositoryInfo {
            path: path_string.clone(),
            index: n,
            property: None,
            kind: "origin".to_string(),
            message: "Debian".to_string(),
        });
    }
    expected_infos.sort();

    let mut infos = check_repositories(&[file], &DebianCodename::Bullseye, &apt_lists_dir);
    infos.sort();

    assert_eq!(infos, expected_infos);
    Ok(())
}

#[test]
fn test_get_cached_origin() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");
    let apt_lists_dir: PathBuf = test_dir.clone().join("lists");

    let pve_list = read_dir.join("pve.list");
    let mut file = APTRepositoryFile::new(pve_list)?.unwrap();
    file.parse()?;

    let origins = [
        Some("Debian".to_string()),
        Some("Debian".to_string()),
        Some("Proxmox".to_string()),
        None, // no cache file exists
        None, // no cache file exists
        Some("Debian".to_string()),
    ];

    assert_eq!(file.repositories.len(), origins.len());

    for (n, repo) in file.repositories.iter().enumerate() {
        assert_eq!(repo.get_cached_origin(&apt_lists_dir)?, origins[n]);
    }

    Ok(())
}

#[test]
fn test_standard_repositories() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");

    // Expected lists are taken straight from the declarative table so adding a Ceph release or
    // host-product channel does not need a parallel edit here.
    let host_pve = HostProduct::Pve;
    let host_pbs = HostProduct::Pbs;
    let pve_bookworm = standard_repos_offered_for(&host_pve, &DebianCodename::Bookworm);
    let mut expected = standard_repos_offered_for(&host_pve, &DebianCodename::Trixie);

    // Mutate a row's status by handle so reorderings or insertions in the table don't silently
    // shift indices and pass against the wrong row.
    let set_status = |repos: &mut [APTStandardRepository],
                      handle: APTRepositoryHandle,
                      status: Option<bool>| {
        let row = repos
            .iter_mut()
            .find(|r| r.handle == handle)
            .unwrap_or_else(|| panic!("no expected row for handle {handle}"));
        row.status = status;
    };

    let absolute_suite_list = read_dir.join("absolute_suite.list");
    let mut file = APTRepositoryFile::new(absolute_suite_list)?.unwrap();
    file.parse()?;

    // On Bookworm, no Ceph Squid is offered.
    let std_repos = standard_repositories(&[file], &host_pve, &DebianCodename::Bookworm);
    assert_eq!(std_repos, pve_bookworm);

    let absolute_suite_list = read_dir.join("absolute_suite.list");
    let mut file = APTRepositoryFile::new(absolute_suite_list)?.unwrap();
    file.parse()?;

    let std_repos = standard_repositories(&[file], &host_pve, &DebianCodename::Trixie);
    assert_eq!(std_repos, expected);

    // PBS view of a PVE-flavored fixture: host-product handles don't match
    // each other's URIs, so all three statuses stay None. (Use the absolute
    // suite list since it has no PVE URIs anyway and we just want to
    // exercise the host-product cross-check.)
    let absolute_suite_list = read_dir.join("absolute_suite.list");
    let mut file = APTRepositoryFile::new(absolute_suite_list)?.unwrap();
    file.parse()?;
    let std_repos = standard_repositories(&[file], &host_pbs, &DebianCodename::Bookworm);
    let pbs_expected = standard_repos_offered_for(&host_pbs, &DebianCodename::Bookworm);
    assert_eq!(std_repos, pbs_expected);

    // ---
    // Ceph Squid detection on Trixie.

    let pve_alt_list = read_dir.join("ceph-squid-trixie.list");
    let mut file = APTRepositoryFile::new(pve_alt_list)?.unwrap();
    file.parse()?;
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_ENTERPRISE, Some(true));
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_NO_SUBSCRIPTION, Some(true));
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_TEST, Some(true));
    let std_repos = standard_repositories(&[file], &host_pve, &DebianCodename::Trixie);
    assert_eq!(std_repos, expected);

    let pve_alt_list = read_dir.join("ceph-squid-nosub-trixie.list");
    let mut file = APTRepositoryFile::new(pve_alt_list)?.unwrap();
    file.parse()?;
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_ENTERPRISE, None);
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_TEST, None);
    let std_repos = standard_repositories(&[file], &host_pve, &DebianCodename::Trixie);
    assert_eq!(std_repos, expected);

    let pve_alt_list = read_dir.join("ceph-squid-enterprise-trixie.list");
    let mut file = APTRepositoryFile::new(pve_alt_list)?.unwrap();
    file.parse()?;
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_ENTERPRISE, Some(true));
    set_status(&mut expected, APTRepositoryHandle::CEPH_SQUID_NO_SUBSCRIPTION, None);
    let std_repos = standard_repositories(&[file], &host_pve, &DebianCodename::Trixie);
    assert_eq!(std_repos, expected);

    Ok(())
}

#[test]
fn test_get_current_release_codename() -> Result<(), Error> {
    let codename = get_current_release_codename()?;

    // If this fails due to you building on another release, e.g. when bootstrapping the next major
    // release, you can changes this but should at least ensure that:
    //
    // - The defined standard repos match what's available for that target release.
    // - The format to write out repos is matching the default format of that release. E.g. deb822
    //   .sources for trixie and newer, single line entries .list for older releases.
    // - All known DebianCodenames are implemented.

    assert!(codename == DebianCodename::Trixie);

    Ok(())
}

/// Legacy `enterprise.proxmox.com/debian` URIs (no `/pve` suffix) still in user configs must
/// keep mapping to the PVE handle, even though we now write the `/pve`-suffixed canonical form.
#[test]
fn test_pve_legacy_uri_still_detected() -> Result<(), Error> {
    use proxmox_apt::repositories::APTRepositoryHandleImpl;
    use proxmox_apt_api_types::{
        APTRepository, APTRepositoryFileType, APTRepositoryOption, APTRepositoryPackageType,
    };

    let host_pve = HostProduct::Pve;
    let suite = DebianCodename::Bullseye;

    let legacy_enterprise = APTRepository {
        types: vec![APTRepositoryPackageType::Deb],
        uris: vec!["https://enterprise.proxmox.com/debian".to_string()],
        suites: vec!["bullseye".to_string()],
        components: vec!["pve-enterprise".to_string()],
        options: vec![APTRepositoryOption {
            key: "Signed-By".into(),
            values: vec!["/usr/share/keyrings/proxmox-archive-keyring.gpg".into()],
        }],
        comment: String::new(),
        file_type: APTRepositoryFileType::List,
        enabled: true,
    };
    assert!(
        APTRepositoryHandle::ENTERPRISE.is_referenced_by(&legacy_enterprise, &host_pve, &suite),
        "PVE legacy enterprise URI without /pve must still be detected as Enterprise"
    );

    let legacy_nosub = APTRepository {
        types: vec![APTRepositoryPackageType::Deb],
        uris: vec!["http://download.proxmox.com/debian".to_string()],
        suites: vec!["bullseye".to_string()],
        components: vec!["pve-no-subscription".to_string()],
        options: vec![],
        comment: String::new(),
        file_type: APTRepositoryFileType::List,
        enabled: true,
    };
    assert!(
        APTRepositoryHandle::NO_SUBSCRIPTION.is_referenced_by(&legacy_nosub, &host_pve, &suite),
        "PVE legacy no-subscription URI without /pve must still be detected"
    );

    let canonical_enterprise = APTRepository {
        types: vec![APTRepositoryPackageType::Deb],
        uris: vec!["https://enterprise.proxmox.com/debian/pve".to_string()],
        suites: vec!["bullseye".to_string()],
        components: vec!["pve-enterprise".to_string()],
        options: vec![],
        comment: String::new(),
        file_type: APTRepositoryFileType::List,
        enabled: true,
    };
    assert!(
        APTRepositoryHandle::ENTERPRISE.is_referenced_by(&canonical_enterprise, &host_pve, &suite),
        "canonical PVE enterprise URI must still be detected"
    );

    let host_pbs = HostProduct::Pbs;
    assert!(
        !APTRepositoryHandle::ENTERPRISE.is_referenced_by(&legacy_enterprise, &host_pbs, &suite),
        "PVE URI must not be recognized as a PBS Enterprise repo"
    );

    Ok(())
}

/// PVE 8 / PBS 3 / PMG 8 wrote the Test channel component as `pvetest` / `pbstest` / `pmgtest`;
/// trixie standardized to the hyphenated form. Upgraded hosts keep the legacy spelling in
/// `sources.list` until apt rewrites it, so detection must still recognize it as the Test repo.
#[test]
fn test_legacy_unhyphenated_test_component_still_detected() -> Result<(), Error> {
    use proxmox_apt::repositories::APTRepositoryHandleImpl;
    use proxmox_apt_api_types::{APTRepository, APTRepositoryFileType, APTRepositoryPackageType};

    let suite = DebianCodename::Bookworm;
    let suite_str = suite.to_string();

    for (host, slug) in [
        (HostProduct::Pve, "pve"),
        (HostProduct::Pbs, "pbs"),
        (HostProduct::Pmg, "pmg"),
    ] {
        let legacy_test = APTRepository {
            types: vec![APTRepositoryPackageType::Deb],
            uris: vec![format!("http://download.proxmox.com/debian/{slug}")],
            suites: vec![suite_str.clone()],
            components: vec![format!("{slug}test")],
            options: vec![],
            comment: String::new(),
            file_type: APTRepositoryFileType::List,
            enabled: true,
        };
        assert!(
            APTRepositoryHandle::TEST.is_referenced_by(&legacy_test, &host, &suite),
            "legacy component '{slug}test' must still map to the Test handle for {host:?}",
        );
    }
    Ok(())
}

/// PVE 8's unhyphenated `pvetest` (and PBS/PMG equivalents) must be rewritten to their kebab-case
/// form on the next write so a `apt update` against the PVE 9 archive (which no longer hosts the
/// legacy path) does not 404.
#[test]
fn test_legacy_unhyphenated_test_component_canonicalized_on_write() -> Result<(), Error> {
    use proxmox_apt::repositories::canonicalize_components_to_standard;
    use proxmox_apt_api_types::{APTRepository, APTRepositoryFileType, APTRepositoryPackageType};

    let suite = DebianCodename::Bookworm;
    let suite_str = suite.to_string();

    for (host, slug) in [
        (HostProduct::Pve, "pve"),
        (HostProduct::Pbs, "pbs"),
        (HostProduct::Pmg, "pmg"),
    ] {
        let mut legacy_test = APTRepository {
            types: vec![APTRepositoryPackageType::Deb],
            uris: vec![format!("http://download.proxmox.com/debian/{slug}")],
            suites: vec![suite_str.clone()],
            components: vec![format!("{slug}test")],
            options: vec![],
            comment: String::new(),
            file_type: APTRepositoryFileType::List,
            enabled: true,
        };
        let changed = canonicalize_components_to_standard(&mut legacy_test, &host, &suite);
        assert!(changed, "{slug}test should report a change after canonicalize");
        assert_eq!(
            legacy_test.components,
            vec![format!("{slug}-test")],
            "{slug}test must be rewritten to '{slug}-test' for {host:?}",
        );

        // Idempotent: a second pass on the already-canonical components must report no change.
        let again = canonicalize_components_to_standard(&mut legacy_test, &host, &suite);
        assert!(!again, "canonicalize on an already-canonical repo must be a no-op");
    }
    Ok(())
}

/// PDM/PMG must offer the full host-product channel set; pins a future refactor against silent drop.
#[test]
fn test_pdm_pmg_host_products_offer_full_channel_set() -> Result<(), Error> {
    use proxmox_apt::repositories::APTRepositoryHandleImpl;

    let suite = DebianCodename::Trixie;

    for (host_product, slug) in [(HostProduct::Pdm, "pdm"), (HostProduct::Pmg, "pmg")] {
        let offered = standard_repositories(&[], &host_product, &suite);
        let handles: Vec<APTRepositoryHandle> = offered.iter().map(|r| r.handle.clone()).collect();
        assert_eq!(
            handles,
            vec![
                APTRepositoryHandle::ENTERPRISE,
                APTRepositoryHandle::NO_SUBSCRIPTION,
                APTRepositoryHandle::TEST,
            ],
            "{host_product:?} should offer the three host-product channels in canonical order",
        );

        for r in &offered {
            assert!(
                r.handle.repo_type().is_none(),
                "{host_product:?} should only offer host-product handles, got {:?}",
                r.handle,
            );
        }

        let enterprise = APTRepositoryHandle::ENTERPRISE
            .to_repository(&host_product, &suite)
            .expect("Enterprise must be offered on every host product");
        assert_eq!(enterprise.components, vec![format!("{slug}-enterprise")]);
        assert_eq!(
            enterprise.uris,
            vec![format!("https://enterprise.proxmox.com/debian/{slug}")],
        );
        let path = APTRepositoryHandle::ENTERPRISE
            .file_path(&host_product, &suite)
            .expect("Enterprise has a path on every host product");
        assert_eq!(
            path,
            format!("/etc/apt/sources.list.d/{slug}-enterprise.sources"),
        );
    }
    Ok(())
}
