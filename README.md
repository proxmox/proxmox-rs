# Local cargo config

This repository ships with a `.cargo/config.toml` that replaces the crates.io
registry with packaged crates located in `/usr/share/cargo/registry`.

A similar config is also applied building with `dh_cargo`. Cargo.lock needs to
be deleted when switching between packaged crates and crates.io, since the
checksums are not compatible.

To reference new dependencies (or updated versions) that are not yet packaged,
the dependency needs to point directly to a path or git source.

# Quickly installing all packages from apt

To a void too many manual installations when `mk-build-deps` etc. fail, a quick
way to install all the main packages of this workspace is to run:

    # apt install $(make list-packages)

# Steps for Releases

1. Run `./bump.sh <CRATE> [patch|minor|major|<VERSION>]`
   - Fill out changelog
   - Confirm bump commit
2. Build the debian source control package with `make <crate>-dsc` to refresh
   `debian/control`.
   - Don't forget to `git commit --amend` apply the updated d/control to the
     bump commit.
3. Build the actual packages with `make clean <crate>-deb`.

# Adding Crates

1. At the top level:
   - Generate the crate: `cargo new --lib the-name`
   - Sort the crate into `Cargo.toml`'s `workspace.members`

2. In the new crate's `Cargo.toml`:
   - In `[package]` set:

         authors.workspace = true
         edition.workspace = true
         exclude.workspace = true
         homepage.workspace = true
         license.workspace = true
         repository.workspace = true
         rust-version.workspace = true

     If a separate ``exclude`` is need it, separate it out as its own
     block above the inherited fields.
   - Add a meaningful `description`
   - Copy `debian/copyright` and `debian/debcargo.toml` from another subcrate.

3. In the new crate\'s `lib.rs`, add the following preamble on top:

       #![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

4. Ideally (but optionally) in the new crate\'s `lib.rs`, add the following
   preamble on top as well:

       #![deny(unsafe_op_in_unsafe_fn)]
       #![deny(missing_docs)]

# Adding a new Dependency

1. At the top level:
   - Add it to `[workspace.dependencies]` specifying the version and any
     features that should be enabled throughout the workspace
2. In each member\'s `Cargo.toml`:
   - Add it to the desired dependencies section with `workspace = true` and no
     version specified.
   - If this member requires additional features, add only the extra features
     to the member dependency.

# Updating a Dependency\'s Version

1. At the top level:
   - Bump the version in `[workspace.dependencies]` as desired.
   - Check for deprecations or breakage throughout the workspace.

# Notes on Workspace Inheritance

Common metadata (like authors, license, ..) are inherited throughout the
workspace. If new fields are added that are identical for all crates, they
should be defined in the top-level `Cargo.toml` file\'s `[workspace.package]`
section, and inherited in all members explicitly by setting `FIELD.workspace =
true` in the member\'s `[package]` section.

Dependency information is also inherited throughout the workspace, allowing a
single dependency specification in the top-level `Cargo.toml` file to be used
by all members.

Some restrictions apply:

- features can only be added in members, never removed (this includes
  `default_features = false`!)
  - the base feature set at the workspace level should be the minimum (possibly
    empty!) set required by all members
- workspace dependency specifications cannot include `optional`
  - if needed, the `optional` flag needs to be set at the member level when
    using a workspace dependency

# Working with *other* projects while changing to *single crates here*

When crates from this workspace need changes caused by requirements in projects
*outside* of this repository, it can often be annoying to keep building and
installing `.deb` files.

Additionally, doing so often requires complete rebuilds as cargo will not pick
up *file* changes of external dependencies.

One way to fix this is by actually changing the version. Since we cut away
anything starting at the first hyphen in the version, we need to use a `+`
(build metadata) version suffix.

Eg. turn `5.0.0` into `5.0.0+test8`.

There are 2 faster ways:

## Adding a `#[patch.crates-io]` section to the other project.

Note, however, that this requires *ALL* crates from this workspace to be listed,
otherwise multiple conflicting versions of the same crate AND even the same
numerical *version* might be built, causing *weird* errors.

The advantage, however, is that `cargo` will pick up on file changes and rebuild
the crate on changes.

## An in-between: system extensions

An easy way to quickly get the new package "installed" *temporarily*, such that
real apt package upgrades are unaffected is as a system-extension.

The easiest way — if no other extensions are used — is to just symlink the
`extensions/` directory to `/run` as root via:

```
# ln -s ${THIS_DIR}/extensions /run/extensions
```

This does not persist across reboots.
(Note: that the `extensions/` directory does not need to exist for the above to
work.)

Once this is done, trying a new version of a crate works by:

1. Bump the version: eg. `5.0.0+test8` -> `5.0.0+test9`
   While this is technically optional (the sysext would then *replace*
   (temporarily) the installed version as long as the sysext is active), just
   like with `.deb` files, not doing this causes `cargo` to consider the crate
   to be unchanged and it will not rebuild its code.
2. here:    `$ make ${crate}-sysext`    (rebuilds `extensions/${crate}.raw`)
3. as root: `# systemd-sysext refresh`  (activates current extensions images)
4. in the other project: `$ cargo update && cargo build`

In the last step, cargo sees that there's a newer version of the crate available
and use that.
