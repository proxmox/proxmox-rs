# Shortcut for common operations:

CRATES=proxmox proxmox-api-macro proxmox-sortable-macro

# By default we just run checks:
.PHONY: all
all: check

.PHONY: deb
deb: $(foreach c,$(CRATES), $c-deb)
	echo $(foreach c,$(CRATES), $c-deb)
	lintian build/*.deb

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

%-autopkgtest:
	autopkgtest build/$* build/*.deb -- null
	touch $@

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
	rm -rf build *-deb *-autopkgtest

.PHONY: update
update:
	cargo update

%-upload: %-deb
	cd build; \
	    dcmd --deb rust-$*_*.changes \
	    | grep -v '.changes$$' \
	    | tar -cf- -T- \
	    | ssh -X repoman@repo.proxmox.com upload --product devel --dist buster
