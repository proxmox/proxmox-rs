# Shortcut for common operations:

# see proxmox-backup if we ever want to support other prefixes
CRATES != echo pbs-*/Cargo.toml proxmox-*/Cargo.toml | sed -e 's|/Cargo.toml||g'

# By default we just run checks:
.PHONY: all
all: check

.PHONY: deb
deb: $(foreach c,$(CRATES), $c-deb)
	echo $(foreach c,$(CRATES), $c-deb)
	lintian build/*.deb

.PHONY: dsc
dsc: $(foreach c,$(CRATES), $c-dsc)
	echo $(foreach c,$(CRATES), $c-dsc)
	lintian build/*.dsc

.PHONY: autopkgtest
autopkgtest: $(foreach c,$(CRATES), $c-autopkgtest)

.PHONY: dinstall
dinstall:
	$(MAKE) clean
	$(MAKE) deb
	sudo -k dpkg -i build/librust-*.deb

%-deb:
	./build.sh $*
	touch $@

proxmox-oci-deb:
	TEST_CMD="fakeroot cargo test --all-features --all-targets --release" ./build.sh proxmox-oci
	touch $@

%-dsc:
	BUILDCMD='dpkg-buildpackage -S -us -uc -d' NOTEST=1 ./build.sh $*
	touch $@

%-autopkgtest:
	autopkgtest build/$* build/*.deb -- null
	touch $@

.PHONY: list-packages
list-packages:
	@for p in $(CRATES); do \
		echo "librust-$$p-dev"; \
	done

.PHONY: check
check:
	cargo test

# Run the api-test server, serving the api-test/www/ subdir as 'www' dir over
# http:
.PHONY: apitest
apitest:
	cargo run -p api-test -- api-test/www/

# Prints a diff between the current code and the one rustfmt would produce
.PHONY: fmt
fmt:
	cargo +nightly fmt -- --check

# Doc without dependencies
.PHONY: doc
doc:
	cargo doc --no-deps

.PHONY: clean
clean:
	cargo clean
	rm -rf build/
	rm -f -- *-deb *-dsc *-autopkgtest *.build *.buildinfo *.changes

.PHONY: update
update:
	cargo update

%-upload: %-deb
	cd build; \
	    dcmd --deb rust-$*_*.changes \
	    | grep -v '.changes$$' \
	    | tar -cf "$@.tar" -T-; \
	    cat "$@.tar" | ssh -X repoman@repo.proxmox.com upload --product devel --dist trixie

%-install:
	rm -rf build/install/$*
	mkdir -p build/install/$*
	BUILDDIR=build/install/$* BUILDCMD=/usr/bin/true NOCONTROL=1 ./build.sh "$*" || true
	version="$$(dpkg-parsechangelog -l $*/debian/changelog -SVersion | sed -e 's/-.*//')"; \
	  install -m755 -Dd "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}"; \
	  rm -rf "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}"; \
	  mv "build/install/$*/$*" \
	    "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}"; \
	  mv "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}/debian/cargo-checksum.json" \
	    "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}/.cargo-checksum.json"; \
	  rm -rf "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}/debian" \

.PHONY: install
install: $(foreach c,$(CRATES), $c-install)

%-install-overlay: %-install
	version="$$(dpkg-parsechangelog -l $*/debian/changelog -SVersion | sed -e 's/-.*//')"; \
	  setfattr -n trusted.overlay.opaque -v y \
	    "$(DESTDIR)/usr/share/cargo/registry/$*-$${version}"
	install -m755 -Dd $(DESTDIR)/usr/lib/extension-release.d
	echo 'ID=_any' >$(DESTDIR)/usr/lib/extension-release.d/extension-release.$*

.PHONY: install-overlay
install-overlay: $(foreach c,$(CRATES), $c-install-overlay)

# To make sure a sysext *replaces* a crate, rather than "merging" with it, we
# need to be able to set the 'trusted.overlay.opaque' xattr. Since we cannot do
# this as a user, we utilize `fakeroot` which keeps track of this for us, and
# turn the final directory into an 'erofs' file system image.
#
# The reason is that if a crate gets changed like this:
#
#     old:
#         src/foo.rs
#     new:
#         src/foo/mod.rs
#
# if its /usr/share/cargo/registry/$crate-$version directory was not marked as
# "opaque", the merged file system would end up with both
#
#         src/foo.rs
#         src/foo/mod.rs
#
# together.
#
# See https://docs.kernel.org/filesystems/overlayfs.html
%-sysext:
	fakeroot $(MAKE) $*-sysext-do
%-sysext-do:
	rm -f extensions/$*.raw
	rm -rf build/sysext/$*
	rm -rf build/install/$*
	$(MAKE) DESTDIR=build/sysext/$* $*-install-overlay
	mkdir -p extensions
	mkfs.erofs extensions/$*.raw build/sysext/$*

sysext:
	fakeroot $(MAKE) sysext-do
sysext-do:
	rm -f extensions/proxmox-workspace.raw
	[ -n "$(NOCLEAN)" ] || rm -rf build/sysext/workspace
	$(MAKE) DESTDIR=build/sysext/workspace $(foreach c,$(CRATES), $c-install)
	install -m755 -Dd build/sysext/workspace/usr/lib/extension-release.d
	echo 'ID=_any' >build/sysext/workspace/usr/lib/extension-release.d/extension-release.proxmox-workspace
	mkdir -p extensions
	mkfs.erofs extensions/proxmox-workspace.raw build/sysext/workspace
