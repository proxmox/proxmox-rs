Local cargo config
==================

This repository ships with a ``.cargo/config`` that replaces the crates.io
registry with packaged crates located in ``/usr/share/cargo/registry``.

A similar config is also applied building with dh_cargo. Cargo.lock needs to be
deleted when switching between packaged crates and crates.io, since the
checksums are not compatible.

To reference new dependencies (or updated versions) that are not yet packaged,
the dependency needs to point directly to a path or git source.

Steps for Releases
==================

- Cargo.toml updates:
  - Bump all modified crate versions.
  - Update all the other crates' Cargo.toml to depend on the new versions if
    required, then bump their version as well if not already done.
- Update debian/changelog files in all the crates updated above.
- Build packages with `make deb`.
