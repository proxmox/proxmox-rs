#!/bin/bash

package=$1

if [[ -z "$package" ]]; then
	echo "USAGE:"
	echo -e "\t bump.sh <crate> [patch|minor|major|<version>]"
	echo ""
	echo "Defaults to bumping patch version by 1"
	exit 0
fi

cargo_set_version="$(command -v cargo-set-version)"
if [[ -z "$cargo_set_version" || ! -x "$cargo_set_version" ]]; then
	echo 'bump.sh requires "cargo set-version", provided by "cargo-edit".'
	exit 1
fi

if [[ ! -e "$package/Cargo.toml" ]]; then
	echo "Invalid crate '$package'"
	exit 1
fi

version=$2
if [[ -z "$version" ]]; then
	version="patch"
fi

case "$version" in
	patch|minor|major)
		bump="--bump"
		;;
	*)
		bump=
		;;
esac

cargo_toml="$package/Cargo.toml"
changelog="$package/debian/changelog"

cargo set-version -p "$package" $bump "$version"
version="$(cargo metadata --format-version=1 | jq ".packages[] | select(.name == \"$package\").version" | sed -e 's/\"//g')"
DEBFULLNAME="Proxmox Support Team" DEBEMAIL="support@proxmox.com" dch --no-conf --changelog "$changelog" --newversion "$version-1" --distribution stable
git commit --edit -sm "bump $package to $version-1" Cargo.toml "$cargo_toml" "$changelog"
