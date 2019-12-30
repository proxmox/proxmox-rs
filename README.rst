Versioning
==========

Note that the following applies *per crate*, since each crate is its own debian package:

The first commit after a release should bump the version to the next patch level with a ``-dev.1``
suffix for the crate *and for the proxmox crate*:

    0.1.0 -> 0.1.1-dev.1

It is unlikely that we'll need more than one .dev version, but it may be useful at some point, so
we'll include teh ``.1``.

When releasing a crate, the final commit should be the one stripping the ``-dev`` version and
updating the ``debian/changelog``.
