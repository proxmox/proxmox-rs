#!/bin/sh

set -e

CRATE=$1
BUILDCMD=${BUILDCMD:-"dpkg-buildpackage -b -uc -us"}

mkdir -p build
rm -rf "build/${CRATE}"

debcargo package --config "$(pwd)/${CRATE}/debian/debcargo.toml" --changelog-ready --no-overlay-write-back --directory "$(pwd)/build/${CRATE}" "${CRATE}" "$(dpkg-parsechangelog -l "${CRATE}/debian/changelog" -SVersion | sed -e 's/-.*//')"
cd "build/${CRATE}"
${BUILDCMD}
