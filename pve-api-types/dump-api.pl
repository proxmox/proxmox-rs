#!/usr/bin/env perl

use v5.36;

use lib './generator-lib';
use ApiDump;

# Load api components
use PVE::API2;

# This is used to build the pve-storage-content enum:
use PVE::Storage::Plugin;

# To resolve the formats:
use PVE::JSONSchema;

my $root = PVE::API2->api_dump(undef, 1);
my $formats = {};

ApiDump::collect_formats($root, $formats, \&PVE::JSONSchema::get_format);
#
my $api = {
    root => $root,
    formats => $formats,
    'storage-content-types' => [
        sort keys PVE::Storage::Plugin::valid_content_types('dir')->%*
    ],
};
ApiDump::dump_api(\$api);
