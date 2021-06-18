.PHONY: all
all: check

.PHONY: check
check:
	cargo test --all-features

.PHONY: dinstall
dinstall: deb
	sudo -k dpkg -i build/librust-*.deb

.PHONY: build
build:
	rm -rf build
	rm -f debian/control
	mkdir build
	debcargo package \
	    --config "$(PWD)/debian/debcargo.toml" \
	    --changelog-ready \
	    --no-overlay-write-back \
	    --directory "$(PWD)/build/proxmox-openid" \
	    "proxmox-openid" \
	    "$$(dpkg-parsechangelog -l "debian/changelog" -SVersion | sed -e 's/-.*//')"
	echo system >build/rust-toolchain
	rm -f build/proxmox-openid/Cargo.lock
	find build/proxmox-openid/debian -name '*.hint' -delete
	cp build/proxmox-openid/debian/control debian/control

.PHONY: deb
deb: build
	(cd build/proxmox-openid && CARGO=/usr/bin/cargo RUSTC=/usr/bin/rustc dpkg-buildpackage -b -uc -us)
	lintian build/*.deb

.PHONY: clean
clean:
	rm -rf build *.deb *.buildinfo *.changes *.orig.tar.gz
	cargo clean

upload: deb
	cd build; \
	    dcmd --deb rust-proxmox-openid_*.changes \
	    | grep -v '.changes$$' \
	    | tar -cf "rust-proxmox-openid-debs.tar" -T-; \
	    cat "rust-proxmox-openid-debs.tar" | ssh -X repoman@repo.proxmox.com upload --product devel --dist bullseye; \
	    rm -f rust-proxmox-openid-debs.tar
