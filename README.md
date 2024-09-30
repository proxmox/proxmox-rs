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

- Run `./bump.sh <CRATE> [patch|minor|major|<VERSION>]`
  - Fill out changelog
  - Confirm bump commit
- Build packages with `make <crate>-deb`.
  - Don't forget to commit updated d/control!

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
