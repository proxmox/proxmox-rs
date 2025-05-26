#!/bin/sh

set -e

export CARGO=/usr/bin/cargo
export RUSTC=/usr/bin/rustc

CRATE=$1
BUILDCMD=${BUILDCMD:-"dpkg-buildpackage -b -uc -us"}
BUILDDIR="${BUILDDIR:-"build"}"

mkdir -p "${BUILDDIR}"
echo system >"${BUILDDIR}"/rust-toolchain
rm -rf ""${BUILDDIR}"/${CRATE}"

CONTROL="$PWD/${CRATE}/debian/control"

if [ -e "$CONTROL" ]; then
    # check but only warn, debcargo fails anyway if crates are missing
    dpkg-checkbuilddeps $PWD/${CRATE}/debian/control || true
    [ "x$NOCONTROL" = 'x' ] && rm -f "$PWD/${CRATE}/debian/control"
fi

debcargo package \
    --config "$PWD/${CRATE}/debian/debcargo.toml" \
    --changelog-ready \
    --no-overlay-write-back \
    --directory "$PWD/"${BUILDDIR}"/${CRATE}" \
    "${CRATE}" \
    "$(dpkg-parsechangelog -l "${CRATE}/debian/changelog" -SVersion | sed -e 's/-.*//')"

cd ""${BUILDDIR}"/${CRATE}"
rm -f debian/source/format.debcargo.hint
${BUILDCMD}

# needs all crates build-dependencies, which can be more than what debcargo assembles.
[ "x$NOTEST" = "x" ] && $CARGO test --all-features --all-targets

[ "x$NOCONTROL" = "x" ] && cp debian/control "$CONTROL"
