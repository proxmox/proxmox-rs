#!/bin/bash
#
# Run integration tests for the proxmox_ldap crate.
# At this time, the tests require `glauth` to be available,
# either explicitly passed via $GLAUTH_PATH, or somewhere
# on $PATH.
#
# Tested with glauth v2.1.0

function run_tests {
    # All tests that need glauth running are ignored, so
    # that we can run `cargo test` without caring about them
    # Also, only run on 1 thread, because otherwise
    # glauth would need a separate port for each rurnning test
    exec cargo test -- --ignored --test-threads 1
}


if [ -z ${GLAUTH_BIN+x} ];
then
    GLAUTH_BIN=$(command -v glauth)
    if [ $? -eq 0 ] ;
    then
        export GLAUTH_BIN
    else
        echo "glauth not found in PATH"
        exit 1
    fi
fi

run_tests
