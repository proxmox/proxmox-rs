# Shortcut for common operations:

# By default we just run checks:
.PHONY: all
all: check

.PHONY: check
check:
	cargo test

# Prints a diff between the current code and the one rustfmt would produce
.PHONY: fmt
checkfmt:
	cargo fmt --all -- --check

# Reformat the code (ppply the output of `make checkfmt`)
.PHONY: fmt
fmt:
	cargo fmt --all

# Doc without dependencies
.PHONY: doc
doc:
	cargo doc --no-deps

.PHONY: clean
clean:
	cargo clean

.PHONY: update
update:
	cargo update
