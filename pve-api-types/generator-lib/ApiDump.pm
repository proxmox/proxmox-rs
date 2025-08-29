package ApiDump;

use v5.36;

use builtin qw(reftype refaddr);

use Carp;
use Digest;

# We use "tagged" values (a perl JSON feature) to replace code refs and regex objects with.
# These are the tags:
package Code {
    sub FREEZE { () }
    sub THAW { sub { die "Nope" } }
}
package Regex {
    sub FREEZE { "$_[0]" }
    sub THAW ($class, $serializer, $regex) { qr/$regex/ }
}

# Since the API contains a LOT of references to the same objects, we also use
# json references, but for convenience we utilize tags for this, since we
# already make use of those.
package Ref {
    use builtin 'refaddr';
    sub FREEZE { $_[0]->{path} }
    sub THAW ($class, $serializer, $path) {
        return bless { path => $path }, $class;
    }
}

use JSON ();

# We want a specific encoder configuration:
my sub get_encoder :prototype() () {
    my $json = JSON->new();
    $json->utf8(1);         # sanity
    $json->pretty(1);       # readability
    $json->canonical(1);    # patch-friendliness
    $json->allow_tags(1);   # deal with code refs and regexes, also duplicates
    $json->space_before(0); # fix 'pretty'
    return $json;
}

# JSON pointers needs some escaping, although no instance where this is
# required exists in PVE code...
my sub escape_jsonptr ($component) {
    $component =~ s|~|~0|g;
    $component =~ s|/|~1|g;
    return $component;
}

# And the other way round.
my sub un_jsonptr ($ptr) {
    $ptr =~ s|~1|/|g;
    $ptr =~ s|~0|~|g;
    return $ptr;
}

# Follow a json pointer given a path and a root object.
my sub follow_jsonptr ($ptr, $root) {
    my $at = $root;
    $ptr =~ s|^/||;
    for my $component (split(m|/|, $ptr)) {
        $component = un_jsonptr($component);
        my $r = ref($at) || '<scalar>';
        if ($r eq 'HASH') {
            $at = $at->{$component};
        } elsif ($r eq 'ARRAY') {
            die "non-numeric index into array: '$component'\n" if $component !~ /^[0-9]+$/;
            $at = $at->[int($component)];
        } else {
            die "json pointer indexing into object of type $r at component '$component'\n";
        }
    }
    return $at;
}

# Prepare a `Ref` for a hash or array value.
#
# - $value is a *reference*.
# - $dedup is our stash of Ref objects.
# - $path is the location of where `$$value` was encountered.
my sub record_dedup ($value, $dedup, $path) {
    $dedup->{refaddr($$value)} = bless {
        value => $$value,
        path => $path,
    }, 'Ref';
}

# Prepare for a dump. This modifies the data IN-PLACE!
#
# This will replace
# - CODE refs with a `Code` instance.
# - Regexps with a `Regex` instance.
# - References to already-encountered objects with `Ref` instances.
#
# Parameters:
#
# - $value is a *reference*, in order to do in-place modifications.
# - The `$dedup` hash maps `refaddr`s to `Ref` objects.
# - The `$path` is tracked in order to generate `Ref` objects.
my sub prepare_to_dump ($value, $dedup = {}, $path = '') {
    my $r = ref($$value);
    if ($r eq 'HASH' || $r eq 'ARRAY') {
        if (defined(my $replacement = $dedup->{refaddr($$value)})) {
            $$value = $replacement;
            return;
        }
    }

    if ($r eq 'HASH') {
        __SUB__->(\($$value)->{$_}, $dedup, "$path/".escape_jsonptr($_)) for sort keys %$$value;
        record_dedup($value, $dedup, $path);
    } elsif ($r eq 'ARRAY') {
        __SUB__->(\($$value)->[$_], $dedup, "$path/".escape_jsonptr($_)) for 0..$#$$value;
        record_dedup($value, $dedup, $path);
    } elsif ($r eq 'CODE') {
        $$value = bless {}, 'Code';
    } elsif ($r eq 'Regexp') {
        $$value = bless $$value, 'Regex';
    }
}

# Opposite of the above: Finish loading the API dump IN-PLACE!
#
# The `THAW` methods of `Code` and `Regex` already fixed up those, but we still
# need to deal with `Ref`s, since we wouldn't have access to the `$root` node
# if we used a "filter" at load-time.
#
# This will replace all `Ref` objects with whatever they reference.
#
# - $value is a *reference* in order to do in-place modification.
# - $root points to the root object to facilitate json pointer lookups.
my sub process_dumped ($value, $root) {
    my $r = ref($$value) or return;

    if ($r eq 'HASH') {
        __SUB__->(\($$value)->{$_}, $root) for keys %$$value;
    } elsif ($r eq 'ARRAY') {
        __SUB__->(\($$value)->[$_], $root) for 0..$#$$value;
    } elsif ($r eq 'Ref') {
        $$value = follow_jsonptr(($$value)->{path}, $root);
        return __SUB__->($value, $root);
    } elsif ($r eq 'Regexp') {
        # ok
    } elsif ($r eq 'CODE') {
        # ok
    } else {
        die "parsed invalid reference out of dumped json: $r\n";
    }
}

# Main entry point to dump an API as JSON.
sub dump_api : prototype($) ($api) {
    prepare_to_dump($api);
    print get_encoder()->encode($$api);
}

# Main entry point to load an API dump from a file.
sub load_api : prototype($) ($file) {
    open my $fh, '<', $file
        or die "failed to open $file: $!\n";
    my $data = do { local $/ = undef; <$fh> };

    $data = get_encoder()->decode($data);
    process_dumped(\$data, $data);
    return $data;
}

# This recurses through a schema and for every found *string* schema with a
# *named* 'format', collects the format (looked up via the `$lookup` sub) into
# the `$formats` hash.
my sub collect_formats_from_schema : prototype($$$) ($schema, $formats, $lookup) {
    return if !%$schema;

    if (defined($schema->{alias})) {
        return;
    } elsif (defined(my $properties = $schema->{properties})) {
        __SUB__->($properties->{$_}, $formats, $lookup) for keys %$properties;
        return;
    } elsif (defined(my $additional = $schema->{additionalProperties})) {
        if (ref($additional) && ref($additional) eq 'HASH') {
            __SUB__->($additional, $formats, $lookup);
        }
        return;
    } elsif (defined(my $items = $schema->{items})) {
        return __SUB__->($items, $formats, $lookup);
    } elsif (exists($schema->{enum})) {
        # enum string types sometimes miss their `type` but they cannot have a
        # format, so just skip it
        return;
    }

    my $ty = $schema->{type};

    if (!defined($ty)) {
        # QUIRKS:

        if ($schema->{format} && $schema->{format} eq 'string') {
            warn "format used instead of type...\n";
            return;
        }

        warn "bad schema with missing type\n";
        return;
    }

    if ($ty eq 'string') {
        my $format_def = $schema->{format};

        return if !defined($format_def);

        if (!ref($format_def)) {
            $format_def =~ s/-a?list$//;
            if (!$formats->{$format_def}) {
                my $fmt = $lookup->($format_def);
                $formats->{$format_def} = $fmt;
                if (ref($fmt) eq 'HASH') {
                    __SUB__->($fmt->{$_}, $formats, $lookup) for keys %$fmt;
                }
            }
        } elsif (ref($format_def) eq 'HASH') {
            __SUB__->($format_def->{$_}, $formats, $lookup) for keys %$format_def;
        }
    }
}

# Iterate through an API dump and collect all the named string formats into the
# `$formats` hash, by performing a lookup via `$lookup` which should simply be
# `\&PVE::JSONSchema::get_format`.
sub collect_formats : prototype($$$) ($api, $formats, $lookup) {
    for my $entry (@$api) {
        if (my $info = $entry->{info}) {
            for my $http_method (keys %$info) {
                my $method = $info->{$http_method};

                my $orig_warn = $SIG{__WARN__};
                my $what = 'parameters';
                local $SIG{__WARN__} = sub {
                    print STDERR "In $http_method method $what on $entry->{path}:\n";
                    local $SIG{__WARN__} = $orig_warn;
                    warn @_;
                };

                my $orig_die = $SIG{__DIE__};
                local $SIG{__DIE__} = sub {
                    print STDERR "In $http_method method $what on $entry->{path}:\n";
                    local $SIG{__DIE__} = $orig_die;
                    Carp::confess @_;
                };

                collect_formats_from_schema($method->{parameters}, $formats, $lookup);
                $what = 'return type';
                collect_formats_from_schema($method->{returns}, $formats, $lookup);
            }
        }
        __SUB__->($entry->{children}, $formats, $lookup);
    }
}

1;
