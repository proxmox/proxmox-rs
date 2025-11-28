package Schema2Rust;

use strict;
use warnings;
use v5.36;

use Carp qw(confess carp croak cluck);
use Data::Dumper;

use Bool;

our $API = 1;
our $LOOKUP_FORMAT = sub { die "lookup_format not initialized!\n" };

my %__derive_default = (
    Debug => 1,
    'serde::Deserialize' => 1,
    'serde::Serialize' => 1,
);
sub derive_default : prototype(@) {
    return {
        %__derive_default,
        map { $_ => 1 } @_
    };
}

my $all_types = {};
my $all_structs = {};
my $all_enums = {};
my $all_methods = {};
my $all_schemas = {};

my $registered_formats = {};
my $registered_derives = {};
my $rename_enum_variant = {};

my $dedup_struct = {};
my $dedup_enum = {};
my $dedup_array_types = {};

our $__err_path = '';
our $__list_format_as_array = 0;

my sub to_doc_comment : prototype($);
my sub strip_doc_comment : prototype($);
my sub handle_def : prototype($$$);
my sub namify_type : prototype($;@);
my sub indent_lines : prototype($$);
sub generate_struct : prototype($$$$);

sub dump {
    print(Dumper($all_types));
}

my sub print_derive : prototype($$) {
    my ($out, $derive) = @_;
    return if !$derive->%*;
    $derive = join(', ', sort keys $derive->%*);
    print {$out} "#[derive($derive)]\n";
}

my sub count_defined_values : prototype($) {
    my ($hash) = @_;
    return scalar(grep { defined($hash->{$_}) } keys $hash->%*);
}

our %API_TYPE_OVERRIDES = ();
sub register_api_override : prototype($$$) {
    my ($rust_type, $path, $value) = @_;
    $API_TYPE_OVERRIDES{"$rust_type:$path"} = $value;
}

our %API_TYPE_EXTENSIONS = ();
sub register_api_extension : prototype($$$) {
    my ($rust_type, $path, $value) = @_;
    $API_TYPE_EXTENSIONS{"$rust_type:$path"} = $value;
}

sub register_api_extensions : prototype($$) {
    my ($rust_type, $extensions) = @_;
    for my $path (keys %$extensions) {
        register_api_extension($rust_type, $path => $extensions->{$path});
    }
}

our $API_TYPE_POS = '';
my sub api_to_string : prototype($$$$$);
# $derive_optional => For structs only, if an api entry contains *only* the 'optional' flag then
#     we can just leave out the schema completely.
sub api_to_string : prototype($$$$$) {
    my ($indent, $out, $api, $derive_optional, $regexes_out) = @_;

    return if !$API || !$api->%*;

    $derive_optional //= '';

    if (my $regexes = $api->{-regexes}) {
        my $regexes_fh = $regexes_out->{fh};
        for my $name (sort keys %$regexes) {
            push $regexes_out->{names}->@*, $name;
            my $value = $regexes->{$name};
            if (ref($value) eq 'ARRAY') {
                my $comma = '';
                print {$regexes_fh} "$name = concat!(";
                for (@$value) {
                    print {$regexes_fh} $comma; $comma = ', ';

                    if (ref($_)) {
                        print {$regexes_fh} $$_;
                    } else {
                        print {$regexes_fh} "r##\"$_\"##";
                    }
                }
                print {$regexes_fh} ");\n";
            } else {
                print {$regexes_fh} "$name = r##\"$value\"##;\n";
            }
        }
    }

    for my $key (sort keys $api->%*) {
        next if $key =~ /^-/;

        my $value = $api->{$key};
        next if !defined($value);

        local $API_TYPE_POS = "$API_TYPE_POS/$key";

        # We need to quote keys with hyphens or reserved keywords:
        my $safe_key = (($key =~ /-/) || ($key eq 'macro') || ($key eq 'ref')) ? "\"$key\"" : $key;

        if (exists($API_TYPE_OVERRIDES{$API_TYPE_POS})) {
            $value = $API_TYPE_OVERRIDES{$API_TYPE_POS};
            next if !defined($value);
        }

        if (!ref($value)) {
            print {$out} "${indent}$safe_key: $value,\n";
        } elsif (my $func = eval { $value->can('TO_API_SCHEMA') }) {
            $value = $value->$func();
            print {$out} "${indent}$safe_key: $value,\n";
        } elsif (ref($value) eq 'HASH') {
            my $next_derive_optional = undef;
            if ($derive_optional eq 'struct' && $key eq 'properties') {
                $next_derive_optional = 'properties';
            } elsif ($derive_optional eq 'properties'
                && count_defined_values($value) == 1
                && $value->{optional})
            {
                next;
            }

            if ($value->%*) {
                my $inner_str = '';
                open(my $inner_fh, '>', \$inner_str);
                api_to_string("$indent    ", $inner_fh, $value, $next_derive_optional, $regexes_out);
                close($inner_fh);

                if (length($inner_str)) {
                    print {$out} "${indent}$safe_key: {\n";
                    print {$out} $inner_str;
                    print {$out} "$indent},\n";
                }
            }
        } else {
            die "unhandled api value type for '$key': ".ref($value)."\n";
        }
    }
    if (defined(my $extra = $API_TYPE_EXTENSIONS{$API_TYPE_POS})) {
        for my $key (sort keys %$extra) {
            if (exists $api->{$key}) {
                warn "api type extension for $API_TYPE_POS.$key already in schema, skipping\n";
                next;
            }
            my $safe_key = (($key =~ /-/) || ($key eq 'macro') || ($key eq 'ref')) ? "\"$key\"" : $key;
            my $value = $extra->{$key};
            print {$out} "${indent}$safe_key: $value,\n";
        }
    }
}

our $regex_test_count = 0;
my sub print_api_string : prototype($$$$) {
    my ($out, $api, $kind, $rust_type_name) = @_;
    return '' if !$API;

    local $API_TYPE_POS = "$rust_type_name:";

    my $api_str = '';
    my $regexes_str = '';
    open(my $api_str_fh, '>', \$api_str);
    open(my $regexes_fh, '>', \$regexes_str);
    my $regexes_out = { fh => $regexes_fh, names => [] };
    api_to_string("    ", $api_str_fh, $api, $kind, $regexes_out);
    close($regexes_fh);
    close($api_str_fh);
    if (length($regexes_str)) {
        print {$out} "const_regex! {\n\n";
        print {$out} $regexes_str;
        print {$out} "\n}\n\n";

        ++$regex_test_count;
        print {$out} "#[test]\n";
        print {$out} "fn test_regex_compilation_${regex_test_count}() {\n";
        print {$out} "    use regex::Regex;\n";
        for my $name ($regexes_out->{names}->@*) {
            print {$out} "    let _: &Regex = &$name;\n";
        }
        print {$out} "}\n";
    }
    if (length($api_str)) {
        print {$out} "#[api(\n${api_str})]\n";
    } else {
        print {$out} "#[api]\n";
    }
}

my sub print_struct : prototype($$$) {
    my ($out, $def, $done_array_types) = @_;

    my @arrays;

    print_api_string($out, $def->{api}, 'struct', $def->{name});

    if (length($def->{description})) {
        print {$out} "$def->{description}\n";
    } elsif ($API) {
        print {$out} "/// Object.\n";
    }
    print_derive($out, $def->{derive});
    print {$out} "pub struct $def->{name} {\n";
    for my $field (sort keys $def->{fields}->%*) {
        my $field_def = $def->{fields}->{$field};
        if ($field_def->{kind} eq 'array-field') {
            push @arrays, $field_def;
        }

        my $name = $field_def->{name};
        my $rust_name = $field_def->{rust_name};
        print {$out} "    $field_def->{description}\n" if length($field_def->{description});
        print {$out} "    $_\n" for $field_def->{attrs}->@*;
        print {$out} "    #[serde(rename = \"$name\")]\n" if $name ne $rust_name;
        print {$out} "    pub $field_def->{rust_name}: $field_def->{type},\n";
        print {$out} "\n";
    }
    if ($def->{additional_properties}) {
        print {$out} "    #[serde(flatten)]\n";
        print {$out} "    pub additional_properties: HashMap<String, Value>,\n";
    }
    print {$out} "}\n";

    for my $array (@arrays) {
        my $type_name = $array->{array_type_name};
        next if $done_array_types->{$type_name};
        $done_array_types->{$type_name} = 1;
        my $count = $array->{array_count};
        print {$out} "generate_array_field! {\n";
        print {$out} "    $type_name [ $count ] :\n";
        print {$out} "    r#\"" . strip_doc_comment($array->{description}) . "\"#,\n";
        my $api_str = '';
        if ($API) {
            open(my $api_str_fh, '>', \$api_str);
            my $regexes_str = '';
            open(my $regexes_fh, '>', \$regexes_str);
            my $regexes_out = { fh => $regexes_fh, names => [] };
            api_to_string(' 'x8, $api_str_fh, $array->{api}, 'array-field', $regexes_out);
        }
        print {$out} "    $array->{'field-type'} => {\n${api_str}";
        print {$out} "    }\n"; # field type and its api doc
        print {$out} "    $array->{name}\n";
        print {$out} "}\n";
    }
    print {$out} "\n\n";
}

sub print_types : prototype($) {
    my ($out) = @_;

    my $done_array_types = {};

    for my $name (sort keys $all_types->%*) {
        my $def = $all_types->{$name};
        my $kind = $def->{kind};
        if ($kind eq 'struct') {
            print_struct($out, $def, $done_array_types);
        } elsif ($kind eq 'enum') {
            print_api_string($out, $def->{api}, 'enum', $def->{name});
            print {$out} "$def->{description}\n" if length($def->{description});
            print_derive($out, $def->{derive});
            print {$out} "pub enum $def->{name} {\n";
            for my $variant ($def->{variants}->@*) {
                my ($orig, $named) = @$variant;
                print {$out} "    #[serde(rename = \"$orig\")]\n" if $named ne $orig;
                print {$out} "    #[default]\n" if $def->{default} && $def->{default} eq $named;
                print {$out} "    /// $orig.\n";
                print {$out} "    $named,\n";
            };
            print {$out} "    /// Unknown variants for forward compatibility.\n";
            print {$out} "    #[serde(untagged)]\n";
            print {$out} "    UnknownEnumValue(FixedString)\n";
            print {$out} "}\n";
            print {$out} "serde_plain::derive_display_from_serialize!($def->{name});\n";
            print {$out} "serde_plain::derive_fromstr_from_deserialize!($def->{name});\n";
            print {$out} "\n";
        } elsif ($kind eq 'schema') {
            my $mod_name = namify_field($name);
            print {$out} "const $name: Schema =\n";
            print {$out} "    proxmox_schema::ArraySchema::new(\n";
            print {$out} "        \"list\",\n";
            print {$out} "        &$def->{items}::API_SCHEMA,\n";
            print {$out} "    ).schema();\n";
            print {$out} "\n";
            print {$out} <<"EOF";
mod $mod_name {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[doc(hidden)]
    pub trait Ser: Sized {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>;
    }

    impl<T: Serialize + for<'a> Deserialize<'a>> Ser for Vec<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            super::stringlist::serialize(&self[..], serializer, &super::$name)
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            super::stringlist::deserialize(
                deserializer,
                &super::$name,
            )
        }
    }

    impl<T: Ser> Ser for Option<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                None => serializer.serialize_none(),
                Some(inner) => inner.ser(serializer),
            }
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use std::fmt;
            use std::marker::PhantomData;

            struct V<T: Ser>(PhantomData<T>);

            impl<'de, T: Ser> serde::de::Visitor<'de> for V<T> {
                type Value = Option<T>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("an optional string")
                }

                fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    T::de(deserializer).map(Some)
                }

                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                    use serde::de::IntoDeserializer;
                    T::de(value.into_deserializer()).map(Some)
                }
            }

            deserializer.deserialize_option(V::<T>(PhantomData))
        }
    }

    pub fn serialize<T, S>(this: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Ser,
    {
        this.ser(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Ser
    {
        T::de(deserializer)
    }
}
EOF
            print {$out} "\n";
        } else {
            die "unhandled kind: $kind\n"
                .'  with properties '.join(', ', sort keys %$def)."\n";
        }
    }
}

my $code_header = <<"CODE";
#[async_trait::async_trait]
impl<T> PveClient for PveClientImpl<T> 
where
    T: HttpApiClient + Send + Sync,
    for<'a> <T as HttpApiClient>::ResponseFuture<'a>: Send
{
CODE
my $code_footer = <<"CODE";
}
CODE

my $trait_header = <<"CODE";
#[async_trait::async_trait]
pub trait PveClient {
CODE
my $trait_footer = <<"CODE";
}
CODE

my sub print_default_impl : prototype($$) {
    my ($out, $method_name) = @_;
    print {$out} "Err(Error::Other(\"$method_name not implemented\"))\n";
    print {$out} "}\n\n";
}

my sub return_expr : prototype($$) ($def, $expr) {
    if ($def->{output_type} eq '()') {
        die "todo: handle returning attributes with .nodata()\n"
            if $def->{'returns-attribs'};
        $expr = "${expr}.nodata()";
    } else {
        $expr = "${expr}.expect_json()";
        if (!$def->{'returns-attribs'}) {
            $expr = "Ok(${expr}?.data)";
        }
    }
    return $expr;
}

my sub format_url : prototype($;$) ($def, $as_ref = 0) {
    if (defined($def->{url_params}) && $def->{url_params}->@*) {
        $as_ref = $as_ref ? '&' : '';

        my $url_with_unnamed_params = url_with_unnamed_params($def->{url});

        my $url = "${as_ref}format!(\"/api2/extjs${url_with_unnamed_params}\"";

        # we have to percent encode string parameter in the url
        for my $url_arg ($def->{url_params}->@*) {
            my ($arg, $def) = @$url_arg;
            my $name = $def->{rust_name};
            my $type = $def->{type};

            if ($type eq '&str') {
                $url .= ",percent_encode(${name}.as_bytes(), percent_encoding::NON_ALPHANUMERIC)";
            } elsif ($type =~ /^u8|u16|u32|u64|u128|i8|i16|i32|i64|i128|usize|isize|f32|f64$/) {
                $url .= ",${name}";
            } else {
                $url .= ",percent_encode(${name}.to_string().as_bytes(), percent_encoding::NON_ALPHANUMERIC)";
            }
        }

        $url .= ")";
    } else {
        return "\"/api2/extjs$def->{url}\"";
    }
}

my sub print_url : prototype($$;$) ($out, $def) {
    print {$out} "    let url = " . format_url($def, 1) . ";\n";
}

my sub print_method_without_body : prototype($$$$$) {
    my ($out, $name, $def, $method, $trait) = @_;

    my $doc = $def->{description};

    if ($doc && length($doc)) {
        print {$out} to_doc_comment($doc)."\n";
    }
    print {$out} "async fn $name(\n";
    print {$out} "    &self,\n";
    for my $url_arg ($def->{url_params}->@*) {
        my ($arg, $def) = @$url_arg;
        print {$out} "    $arg: $def->{type},\n";
    }

    my $input;
    if (defined($input = $def->{input_type})) {
        print {$out} "    params: $input,\n";
        print {$out} ") -> Result<$def->{output_type}, Error> {\n";
        if ($trait) {
            print_default_impl($out, $name);
            return;
        }
        my $ty = $all_types->{$input};
        die "bad parameter type: $ty->{kind}\n" if $ty->{kind} ne 'struct';
        my $fields = $ty->{fields};
        # To ensure we're not missing anything, let'first destruct the parameter struct, this way
        # rustc will complain about missing or unknown parameters:
        print {$out} "    let $input {\n";
        for my $name (sort keys $fields->%*) {
            my $arg = $fields->{$name};
            my $rust_name = $arg->{rust_name};
            print {$out} "    $rust_name: p_$rust_name,\n";
        }
        print {$out} "    } = params;\n";
        print {$out} "\n";
        print {$out} "    let url = &ApiPathBuilder::new(" . format_url($def) . ")\n";
        for my $name (sort keys $fields->%*) {
            my $arg = $fields->{$name};
            my $rust_name = $arg->{rust_name};


            if ($arg->{type} eq 'Option<bool>') {
                print {$out} "        .maybe_bool_arg(\"$name\", p_$rust_name)\n";
            } elsif ($arg->{is_string_list}) {
                print {$out} "        .maybe_list_arg(\"$name\", p_$rust_name)\n";
            } elsif ($arg->{is_string_list_as_array}) {
                if ($arg->{optional}) {
                    print {$out} "        .maybe_list_arg(\"$name\", &p_$rust_name)\n";
                } else {
                    print {$out} "        .list_arg(\"$name\", &p_$rust_name)\n";
                }
            } elsif ($arg->{optional}) {
                print {$out} "        .maybe_arg(\"$name\", &p_$rust_name)\n";
            } else {
                print {$out} "        .arg(\"$name\", &p_$rust_name)\n";
            }
        }
        print {$out} "        .build();\n";
    } elsif (defined($input = $def->{input}) && $input->@*) {
        for my $arg ($input->@*) {
            print {$out} "    $arg->{rust_name}: $arg->{type},\n";
        }
        print {$out} ") -> Result<$def->{output_type}, Error> {\n";
        if ($trait) {
            print_default_impl($out, $name);
            return;
        }
        # print {$out} "    // self.login().await?;\n";
        if (@$input) {
            print {$out} "    let url = &ApiPathBuilder::new(" . format_url($def) . ")\n";
            for my $arg (@$input) {
                my $name = $arg->{name};
                my $rust_name = $arg->{rust_name};

                if ($arg->{type} eq 'Option<bool>') {
                    print {$out} "        .maybe_bool_arg(\"$name\", $rust_name)\n";
                } elsif ($arg->{is_string_list}) {
                    print {$out} "        .maybe_list_arg(\"$name\", &$rust_name)\n";
                } elsif ($arg->{is_string_list_as_array}) {
                    if ($arg->{optional}) {
                        print {$out} "        .maybe_list_arg(\"$name\", &$rust_name)\n";
                    } else {
                        print {$out} "        .list_arg(\"$name\", &$rust_name)\n";
                    }
                } elsif ($arg->{optional}) {
                    print {$out} "        .maybe_arg(\"$name\", &$rust_name)\n";
                } else {
                    print {$out} "        .arg(\"$name\", &$rust_name)\n";
                }
            }
            print {$out} "        .build();\n";
        } else {
            print_url($out, $def);
        }
    } else {
        print {$out} ") -> Result<$def->{output_type}, Error> {\n";
        if ($trait) {
            print_default_impl($out, $name);
            return;
        }

        print_url($out, $def);
    }

    my $call = return_expr($def, "self.0.$method(url).await?");
    print {$out} "    $call\n";

    print {$out} "}\n\n";
}

my sub print_method_with_body : prototype($$$$$) {
    my ($out, $name, $def, $method, $trait) = @_;

    my $doc = $def->{description};

    if ($doc && length($doc)) {
        print {$out} to_doc_comment($doc)."\n";
    }

    print {$out} "async fn $name(\n";
    print {$out} "    &self,\n";
    my $url_params = $def->{url_params};
    for my $url_arg ($url_params->@*) {
        my ($arg, $def) = @$url_arg;
        print {$out} "    $arg: $def->{type},\n";
    }
    print {$out} "    params: $def->{input_type},\n";
    my $output = $def->{output_type} // '()';
    print {$out} ") -> Result<$output, Error> {\n";
    if ($trait) {
        print_default_impl($out, $name);
        return;
    }
    # print {$out} "    // self.login().await?;\n";
    print_url($out, $def);
        my $call = return_expr($def, "self.0.${method}(url, &params).await?");
        print {$out} "    $call\n";
    print {$out} "}\n\n";
}

sub print_implementation : prototype($) {
    my ($out) = @_;

    print {$out} $code_header;
    for my $name (sort keys $all_methods->%*) {
        my $def = $all_methods->{$name};
        my $http_method = $def->{http_method};
        if ($http_method eq 'GET') {
            print_method_without_body($out, $name, $def, 'get', 0);
        } elsif ($http_method eq 'PUT') {
            print_method_with_body($out, $name, $def, 'put', 0);
        } elsif ($http_method eq 'POST') {
            print_method_with_body($out, $name, $def, 'post', 0);
        } elsif ($http_method eq 'DELETE') {
            print_method_without_body($out, $name, $def, 'delete', 0);
        } else {
            print "Method $name: ".Dumper($def);
            warn "TODO: $http_method methods\n";
        }
    }
    print {$out} $code_footer;
}

sub print_trait : prototype($) {
    my ($out) = @_;

    print {$out} $trait_header;
    for my $name (sort keys $all_methods->%*) {
        my $def = $all_methods->{$name};
        my $http_method = $def->{http_method};
        if ($http_method eq 'GET') {
            print_method_without_body($out, $name, $def, 'get', 1);
        } elsif ($http_method eq 'PUT') {
            print_method_with_body($out, $name, $def, 'put', 1);
        } elsif ($http_method eq 'POST') {
            print_method_with_body($out, $name, $def, 'post', 1);
        } elsif ($http_method eq 'DELETE') {
            print_method_without_body($out, $name, $def, 'delete', 1);
        } else {
            print "Method $name: ".Dumper($def);
            warn "TODO: $http_method methods\n";
        }
    }
    print {$out} $trait_footer;
}

sub register_format : prototype($$) {
    my ($name, $data) = @_;

    $registered_formats->{$name} = $data;
}

sub register_derive : prototype($@) {
    my ($name, @derives) = @_;

    my $list = ($registered_derives->{$name} //= []);
    push @$list, @derives;
}

sub register_enum_variant : prototype($$) {
    my ($from, $to) = @_;
    $rename_enum_variant->{$from} = $to;
}

sub derive : prototype($@) {
    my ($type_name, @types) = @_;
    my $def = $all_types->{$type_name}
        or croak "no such type: $type_name\n";
    $def->{derive}->{$_} = 1 for @types;
}

my %warned = (
    type => 1, # this is handled but not explicitly marked
    # we don't care about these:
    completion => 1,
    renderer => 1,
);
my sub warn_unused : prototype($$) {
    my ($type, $kind) = @_;

    for my $property (sort keys $type->%*) {
        next if $warned{$property};
        $warned{$property} = 1;
        if (defined($kind)) {
            warn "Warning: unhandled type property (for $kind): '$property'\n";
        } else {
            warn "Warning: unhandled type property: '$property'\n";
        }
    }
}

sub namify_field : prototype($) {
    my ($arg) = @_;

    my $out = '';

    my $was_uppercase = 1;
    my $was_underscore = 0;
    for my $i (0..(length($arg)-1)) {
        my $ch = substr($arg, $i, 1);
        $ch = ($ch =~ tr/-/_/r);

        my $is_underscore = ($ch eq '_');
        my $is_uppercase = ($ch =~ /^[A-Z]$/);
        if ($is_uppercase) {
            if (!$was_uppercase && !$was_underscore) {
                $out .= '_';
            }
            $out .= lc($ch);
        } elsif ($ch eq '_') {
            if (!$was_underscore) {
                $out .= $ch;
            }
        } else {
            $out .= lc($ch);
        }
        $was_underscore = $is_underscore;
        $was_uppercase = $is_uppercase;
    }

    return 'ty' if $out eq 'type';
    return 'r#macro' if $out eq 'macro';
    return 'r#ref' if $out eq 'ref';

    return $out;
}

sub namify_type : prototype($;@) {
    my $out = '';
    for my $arg_ (@_) {
        confess "namify_type: undef\n" if !defined $arg_;
        my $arg = ($arg_ =~ s/-/_/gr);

        my $was_underscore = 1;
        my $was_lowercase = 0;
        for my $i (0..(length($arg)-1)) {
            my $ch = substr($arg, $i, 1);
            if ($ch eq '_') {
                $was_underscore = 1;
            } elsif ($was_underscore) {
                $was_underscore = 0;
                $out .= ($ch =~ tr/a-z/A-Z/r);
            } elsif ($was_lowercase) {
                $out .= $ch;
            } else {
                $out .= ($ch =~ tr/A-Z/a-z/r);
            }
            $was_lowercase = ($ch =~ /[^A-Z]/);
        }
    }
    return $out;
}

my sub namify_const : prototype($;@) {
    my $out = '';
    for my $arg_ (@_) {
        confess "namify_const: undef\n" if !defined $arg_;
        my $arg = ($arg_ =~ s/-/_/gr);

        $out .= '_' if length($out);

        my $was_underscore = 1;
        my $was_lowercase = 0;
        for my $i (0..(length($arg)-1)) {
            my $ch = substr($arg, $i, 1);
            if ($ch eq '_') {
                $was_underscore = 1;
            } elsif ($was_underscore) {
                $was_underscore = 0;
                $out .= ($ch =~ tr/a-z/A-Z/r);
            } elsif ($was_lowercase) {
                $out .= '_' if $ch =~ /[A-Z]/;
                $out .= ($ch =~ tr/a-z/A-Z/r);
            } else {
                $out .= ($ch =~ tr/a-z/A-Z/r);
            }
            $was_lowercase = ($ch =~ /[^A-Z]/);
        }
    }
    return $out;
}

sub quote_string : prototype($) {
    my ($s) = @_;
    return '"' . ($s =~ s/(["\\])/\\$1/gr) . '"';
}

sub indent_lines : prototype($$) {
    my ($indent, $lines) = @_;

    confess "DARN" if !defined($lines);

    my $out = '';

    for my $line (split(/\n/, $lines)) {
        $out .= "\n" if length($out);
        $out .= "${indent}${line}";
    }

    return $out;
}

sub to_doc_comment : prototype($) {
    my ($text) = @_;

    return '' if !defined($text);

    return indent_lines('/// ', $text);
}

sub strip_doc_comment : prototype($) {
    my ($text) = @_;
    return $text =~ s@^/// @@mgr;
}

my sub type_of : prototype($) {
    my ($schema) = @_;

    my $kind = $schema->{type};

    return $kind if $kind;

    if (exists $schema->{properties}) {
        return 'object';
    } elsif (exists $schema->{items}) {
        return 'array';
    }

    die "missing 'type' and failed to guess\n";
}

my sub get_format : prototype($$) {
    my ($format, $name) = @_;

    my $format_name = $name;
    my $format_kind;
    my $is_string_list = false; # marks formats with an explicit "-list" suffix...
    if (!ref($format)) {
        if ($format =~ /^(.*)-a?(list|opt)$/) {
            ($format, $format_kind) = ($1, $2);
            $is_string_list = bool($format_kind eq 'list');
            $format_kind = 'array' if $is_string_list;
        }

        $format_name = $format;
        my $f = $LOOKUP_FORMAT->($format);
        die "missing format '$format'\n" if !$f;
        $format = $f;
    }

    if (ref($format) eq 'HASH') {
        my $name_hint = namify_type($format_name);
        my $type = generate_struct(
            $name_hint,
            { type => 'object', properties => $format, additionalProperties => 0 },
            {},
            {},
        );
        return {
            format_name => $format_name,
            kind => $format_kind,
            is_string_list => $is_string_list,
            property_string => $type,
        };
    }

    if (ref($format) eq 'CODE') {
        my $info = $registered_formats->{$format_name}
            or die "info for format '$format_name' required\n";

        return {
            kind => $format_kind,
            is_string_list => $is_string_list,
            %$info,
        };
    }

    die "WEIRD FORMAT TYPE: ".ref($format)."\n";
}

sub integer_type : prototype($$) {
    my ($schema, $api_props) = @_;
    my $min = delete $schema->{minimum};
    my $max = delete $schema->{maximum};

    my $ty;
    if (!defined($min) && !defined($max)) {
        $ty = 'i64';
    } elsif (defined($min) && defined($max)) {
        if ($min >= 0) {
            if ($max < 0x100) {
                $ty = 'u8';
            } elsif ($max < 0x10000) {
                $ty = 'u16';
            } elsif ($max <= 0xffffffff) {
                $ty = 'u32';
            } else {
                $ty = 'u64';
            }
        } elsif ($min >= -0x80 && $max < 0x80) {
            $ty = 'i8';
        } elsif ($min >= -0x8000 && $max < 0x8000) {
            $ty = 'i16';
        } elsif ($min >= -0x80000000 && $max < 0x80000000) {
            $ty = 'i32';
        } else {
            $ty = 'i64';
        }
    } elsif (defined($min)) {
        if ($min >= 0) {
            $ty = 'u64';
        } else {
            $ty = 'i64';
        }
    } else { # defined $max
        $ty = 'i64';
    }

    $api_props->{minimum} = $min if defined $min;
    $api_props->{maximum} = $max if defined $max;
    $api_props->{type} = 'Integer';

    return $ty;
}

my sub floatify : prototype($$) {
    my ($hash, $mem) = @_;
    if (defined(my $n = $hash->{$mem})) {
        if ($n !~ /\./) {
            $hash->{$mem} = "$n.0";
        }
    }
}

sub number_type : prototype($$) {
    my ($schema, $api_props) = @_;
    my $min = delete $schema->{minimum};
    my $max = delete $schema->{maximum};

    # FIXME: Can we make any good guesses for using `f32`?
    my $ty = 'f64';

    $api_props->{minimum} = $min if defined $min;
    $api_props->{maximum} = $max if defined $max;

    floatify($api_props, 'minimum');
    floatify($api_props, 'maximum');
    floatify($api_props, 'default');

    return $ty;
}

my sub check_rust_name : prototype($$$) {
    my ($where, $name, $todo) = @_;
    if ($name !~ /^[a-zA-Z_][a-zA-Z_0-9]*$/) {
        confess "bad name in $where: $name\n$todo\n";
    }
}

sub generate_enum : prototype($$;$) {
    my ($name_hint, $schema, $api_props) = @_;

    $api_props //= {};

    my $dedup_key = join("\0", sort $schema->{enum}->@*);
    if (my $name = $dedup_enum->{$dedup_key}) {
        $api_props->{type} = $name;
        return $name;
    }
    # If this is not a duplicate we clash, this happens in `/cluster/resources`
    # where the input parameter `type` and the output field `type` get different
    # enum variants.
    $dedup_enum->{$dedup_key} = $name_hint;
    die "duplicate enum name: '$name_hint'\n" if exists $all_enums->{$name_hint};
    #return $name_hint if exists $all_enums->{$name_hint};

    local $__err_path = "$__err_path.$name_hint";
    my $def = {
        kind => 'enum',
        name => $name_hint,
        attrs => [],
        variants => [],
        derive => derive_default(qw(Clone Copy Eq PartialEq)),
        api => {},
        description => '',
    };

    $all_enums->{$name_hint} = $def;
    die "duplicate type name: '$name_hint'\n" if exists $all_types->{$name_hint};
    $all_types->{$name_hint} = $def;
    $api_props->{type} = $name_hint;

    # Copy so we don't modify the original.
    $schema = { %$schema };

    if (defined(my $description = delete $schema->{description})) {
        $def->{description} = to_doc_comment($description);
    }

    my $variants = $def->{variants};
    my $default = delete $schema->{default};
    my $rust_default;
    for my $variant ((delete $schema->{enum})->@*) {
        my $rust_variant = $rename_enum_variant->{"${name_hint}::$variant"};
        $rust_variant = namify_type($variant) if !defined($rust_variant);
        check_rust_name("enum $name_hint", $rust_variant, 'consider using register_enum_variant');
        push $variants->@*, [$variant, $rust_variant];

        if (defined($default) && $default eq $variant) {
            $rust_default = $rust_variant;
        }
    }
    if (defined($default)) {
        if (defined($rust_default)) {
            $def->{default} = $rust_default;
            $def->{derive} = derive_default(qw(Clone Copy Default Eq PartialEq));
        } else {
            warn "non-existent default enum value '$default'\n" if !defined($rust_default);
        }
    }

    warn_unused($schema, 'enum');

    return $name_hint;
}

my sub generate_array_schema : prototype($$$) {
    my ($name, $description, $items) = @_;

    my $schema;

    die "duplicate schema: $name\n" if exists $all_types->{$name};
    $schema = {
        kind => 'schema',
        type => 'Array',
        description => $description,
        items => $items,
    };

    $all_types->{$name} = $all_schemas->{$name} = $schema;

    return $name;
}

my sub string_type : prototype($$$$) {
    my ($schema, $api_props, $name_hint, $def) = @_;

    $api_props->{type} = 'String';

    if (defined(my $enum = delete $schema->{enum})) {
        confess "enum string type\n";
    }

    if (defined(my $len = delete $schema->{minLength})) {
        $api_props->{min_length} = $len;
    }
    if (defined(my $len = delete $schema->{maxLength})) {
        $api_props->{max_length} = $len;
    }
    if (defined(my $text = delete $schema->{typetext})) {
        $api_props->{type_text} = quote_string($text);
    }
    if (defined(my $text = delete $schema->{default})) {
        $api_props->{default} = quote_string($text);
    }

    # pub enum ApiStringFormat {
    #     /// Enumerate all valid strings
    #     Enum(&'static [EnumEntry]),
    #     /// Use a regular expression to describe valid strings.
    #     Pattern(&'static ConstRegexPattern),
    #     /// Use a schema to describe complex types encoded as string.
    #     PropertyString(&'static Schema),
    #     /// Use a verification function.
    #     VerifyFn(ApiStringVerifyFn),
    # }
    if (defined(my $format = delete $schema->{format})) {
        my $fmt = get_format($format, $name_hint);
        my $kind = $fmt->{kind} // '';

        #if (defined(my $kind = $fmt->{kind})) {
        #    if ($kind eq 'array') {
        #        my $name = $fmt->{format_name};
        #        $api_props->{format} = "&ApiStringFormat::PropertyString(&$name_hint)";
        #        warn "FIXME: FORMAT KIND '$kind'\n";
        #        warn Dumper($fmt);
        #        return 'String';
        #    }
        #}

        if (my $code = $fmt->{code}) {
            $api_props->{format} = "&ApiStringFormat::VerifyFn($code)";
        } elsif (my $regex = $fmt->{regex}) {
            my $re_name = namify_const(${name_hint}, 're');
            $api_props->{-regexes}->{$re_name} = $regex;
            $api_props->{format} = "&ApiStringFormat::Pattern(&$re_name)";
        } elsif ($fmt->{unchecked}) {
            # We don't check this, it'll be an arbitrary string.
        } elsif (my $ty = $fmt->{type}) {
            # raw type, undo
            if ($kind eq 'array') {
                my $array = generate_array_schema(namify_const($name_hint), 'list', $ty);
                $api_props->{format} = "&ApiStringFormat::PropertyString(&$array)";
                my $module_name = namify_field($array);
                push $def->{attrs}->@*, "#[serde(with = \"$module_name\")]";
                $def->{is_string_list} = $fmt->{is_string_list};
                return "Vec<$ty>";
            }

            $api_props->{type} = $ty;
            #$api_props->{format} = "&ApiStringFormat::PropertyString(&${ps}::API_SCHEMA)";
            # Return a "raw" type.
            return $ty;
        } elsif (my $ps = $fmt->{property_string}) {
            $api_props->{format} = "&ApiStringFormat::PropertyString(&${ps}::API_SCHEMA)";
        } else {
            confess "FIXME (string_type format stuff)\n" .Dumper($fmt);
        }
        # if (my $kind = $fmt->{kind}) {
        #     $api_props->{format_fixme} = '"LIST TYPE"';
        # }
    }

    return 'String';
}

my sub is_basic_type : prototype($) ($ty) {
    return 1 if !defined($ty) || $ty eq 'String' || $ty eq 'Integer';
}

my sub array_type : prototype($$$) {
    my ($schema, $api_props, $name_hint) = @_;

    my $def = {
        kind => 'array',
        name => $name_hint,
        rust_name => namify_type($name_hint),
        type => undef, # rust type
        attrs => [],
        api => {},
        optional => undef,
        description => '',
    };
    if (!$schema->{items} && !ref($schema->{format}) && $schema->{format} =~ /-list$/) {
        my $format_name = $schema->{format} =~ s/-list$//r;
        $schema->{items} = {
            description => "List item of type $format_name.",
            format => $format_name,
            type => 'string',
        };
    }

    my $items = delete $schema->{items} or die "missing 'items' in array schema\n";
    my $description = $items->{description};

    handle_def($def, \$items, $name_hint);

    $api_props->{type} = 'Array';
    $api_props->{items} = $def->{api};
    if (
        $description
        && !$items->{description}
        && is_basic_type($api_props->{items}->{type})
    ) {
        $api_props->{items}->{description} = quote_string($description);
    }

    return "Vec<$def->{type}>";
}

my %serde_num = (
    usize => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_usize")]',
    isize => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_isize")]',
    u8 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]',
    u16 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]',
    u32 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]',
    u64 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]',
    i8 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_i8")]',
    i16 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_i16")]',
    i32 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_i32")]',
    i64 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]',
    f32 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_f32")]',
    f64 => '#[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]',
);

sub handle_def : prototype($$$) {
    my ($def, $inout_schema, $name_hint) = @_;

    my $type = type_of($$inout_schema);
    if ($type eq 'string' && $$inout_schema->{enum}) {
        $def->{type} = generate_enum($name_hint, $$inout_schema, $def->{api});
        return;
    }

    # We need the original schema when generating its type.
    # But we use a copy to deal with its attributes to warn about unused things.
    my $orig_schema = $$inout_schema;
    my $schema = { $orig_schema->%* };
    $$inout_schema = $schema;

    if (defined(my $description = delete $schema->{description})) {
        $def->{description} = to_doc_comment($description);
    }

    if ($type eq 'integer') {
        $def->{type} = integer_type($schema, $def->{api});
        if (defined(my $serde = $serde_num{$def->{type}})) {
            push $def->{attrs}->@*, $serde;
        }
        $def->{api}->{default} = delete $schema->{default};
    } elsif ($type eq 'boolean') {
        $def->{type} = 'bool';
        push $def->{attrs}->@*,
            "#[serde(deserialize_with = \"proxmox_serde::perl::deserialize_bool\")]";
        $def->{api}->{default} = bool(delete $schema->{default});
    } elsif ($type eq 'number') {
        $def->{api}->{default} = delete $schema->{default};
        $def->{type} = number_type($schema, $def->{api});
        if (defined(my $serde = $serde_num{$def->{type}})) {
            push $def->{attrs}->@*, $serde;
        }
    } elsif ($type eq 'string') {
        # If type is 'string' but format ends with `-list`, treat as array
        # but only for inputs to endpoints.
        #
        # We want a typed Vec<String>, and since [1] the PVE API does accept
        # actual arrays for parameters with a format ending in `-list`.
        # This is only for inputs though, so we can only have Vec's for
        # inputs, returns are still string lists.
        #
        # [1] pve-common 69d9edcc ("section config: implement array support")
        if ($__list_format_as_array
            && defined($schema->{format})
            && !ref($schema->{format})
            && $schema->{format} =~ /-list$/
        ) {
            $def->{type} = array_type($schema, $def->{api}, $name_hint);
            $def->{is_string_list_as_array} = true;
        } else {
            $def->{type} = string_type($schema, $def->{api}, $name_hint, $def);
        }
    } elsif ($type eq 'object') {
        $def->{type} = generate_struct($name_hint, $orig_schema, {}, $def->{api});
        # generate_struct uses the original schema and warns by itself
        return;
    } elsif ($type eq 'array') {
        $def->{type} = array_type($schema, $def->{api}, $name_hint);
    } else {
        die "unhandled field type: $type\n";
    }

    warn_unused($schema, $type);
}

my sub make_struct_field : prototype($$$$) {
    my ($struct_name, $name, $rust_name, $inout_schema) = @_;
    local $__err_path = "$__err_path.$name";

    my $def = {
        kind => 'field',
        struct => $struct_name,
        name => $name,
        rust_name => $rust_name,
        type => undef, # rust type
        attrs => [],
        api => {},
        optional => undef,
        description => '',
    };


    # in perl the `optional` property is declared in the type's schema but it's not actually part
    # of the type but part of the object-schema so pull it out early to clear the warning:
    my $optional = bool(delete $$inout_schema->{optional});
    handle_def($def, $inout_schema, namify_type($struct_name, $name));

    #my $schema = $$inout_schema;
    # (in case the type was already an option type, don't duplicate the `Option<>`)
    if ($optional && !$def->{optional}) {
        $def->{type} = "Option<$def->{type}>";
        $def->{api}->{optional} = $def->{optional} = true;
        push $def->{attrs}->@*, "#[serde(default, skip_serializing_if = \"Option::is_none\")]";
    }

    return $def;
}

my sub make_struct_array_field : prototype($$$$$$) {
    my ($struct_name, $base_name, $base_rust_name, $count, $inout_schema, $dedup_key) = @_;
    local $__err_path = "$__err_path.${base_name}[]";

    if (defined(my $deduped = $dedup_array_types->{$dedup_key})) {
        return $deduped;
    }

    my $array_type_name = namify_type(
        namify_field($struct_name) . '_' . namify_field($base_name) . '_array'
    );
    my $def = {
        kind => 'array-field',
        struct => $struct_name,
        name => $base_name,
        # FIXME: We cannot just cut off the number because qemu for instance has `numa` and `numaX`
        # but we also don't want to use `numas`...
        rust_name => $base_rust_name,
        type => undef, # rust type
        attrs => [],
        api => {
            '-array-field' => 1,
            description => quote_string($$inout_schema->{description}),
        },
        optional => undef,
        description => '',
        array_type_name => undef,
        array_count => $count,
    };

    # in perl the `optional` property is declared in the type's schema but it's not actually part
    # of the type but part of the object-schema so pull it out early to clear the warning:
    my $optional = bool(delete $$inout_schema->{optional});
    die "array of non-optional values not supported\n" if !$optional;

    handle_def($def, $inout_schema, namify_type($struct_name, $base_name));

    $def->{array_type_name} = $array_type_name;

    #$def->{type} = "Option<$def->{type}>";
    $def->{'field-type'} = $def->{type};
    $def->{api}->{type} = $def->{type};
    $def->{type} = $array_type_name;
    $def->{optional} = true;
    push $def->{attrs}->@*, "#[serde(flatten)]";

    $dedup_array_types->{$dedup_key} = $def;
    return $def;
}

sub is_array : prototype($$) {
    my ($properties, $field) = @_;

    return if $field !~ /^(.*\D)(\d+)$/;

    my ($base, $id) = ($1, $2);
    my $indices = { $id => 1 };
    my $max = $id;

    my $re = qr/^\Q$base\E(\d+)$/;
    for my $key (keys $properties->%*) {
        next if $key !~ $re;
        $indices->{$1} = 1;
        $max = $1 if $1 > $max;
    }

    my $count = scalar(keys $indices->%*);
    return if $count <= 1 || ($count != ($max + 1));

    return ($base, $count);
}

sub generate_struct : prototype($$$$) {
    my ($name_hint, $schema, $extra, $api_props) = @_;
    die "no struct name defined, unnamed parameter list?\n" if !defined($name_hint);
    local $__err_path = "$__err_path => struct $name_hint";

    my $properties = $schema->{properties};

    if (!$properties || !$properties->%*) {
        # Special case:
        # {
        #   type => 'object',
        #   properties => {}, # empty
        #   additionalProperties => {SCHEMA},
        # }
        # does not actually become a struct, but a `HashMap<String, TY>`
        my $additional = $schema->{additionalProperties};
        if (ref($additional)) {
            my $name = generate_struct($name_hint, $additional, $extra, {});
            return "HashMap<String, $name>";
        }
    }

    if (!$properties) {
        # default is 1 urrrgh
        if (!$schema->{additionalProperties}) {
            warn "no 'properties' in object schema, using serde_json::Value\n";
            $api_props->{type} = 'Object';
            $api_props->{properties} = '{}';
            if (defined(my $description = $schema->{description})) {
                $api_props->{description} = quote_string($description);
            } else {
                $api_props->{description} = quote_string("FIXME: missing description in PVE");
            }
            return 'serde_json::Value';
        }
    } else {
        if (my $name = $dedup_struct->{$properties}) {
            $api_props->{type} = $name;
            return $name;
        }
        $dedup_struct->{$properties} = $name_hint;
    }
    $api_props->{type} = $name_hint;
    return $name_hint if exists $all_structs->{$name_hint};

    my $def = {
        kind => 'struct',
        name => $name_hint,
        fields => {},
        derive => derive_default(),
        description => '',
        api => {},
    };
    $all_structs->{$name_hint} = $def;
    die "duplicate type name: '$name_hint'\n" if exists $all_types->{$name_hint};
    $all_types->{$name_hint} = $def;

    # Copy so we don't modify the original.
    $schema = { %$schema };
    $properties = delete($schema->{properties}) // {} ;
    # Copy so we don't modify the original.
    $properties = { %$properties };

    # Can't do this here if we use the original for deduping, since the struct may be reused
    # somewhere without URL parameters, too...
    # if (my $skip = $extra->{skip}) {
    #     # delete skipped properties
    #     delete $properties->{$_} for @$skip;
    # }

    if (defined(my $description = delete $schema->{description})) {
        $def->{description} = to_doc_comment($description);
    }

    my $key_alias_info;

    my @array_bases;
    PROPERTY: for my $field_name (sort keys $properties->%*) {
        if (my $key_alias = $properties->{$field_name}->{keyAlias}) {
            # Legacy property string stuff...
            if (my $existing = $key_alias_info->{key_alias}) {
                die "conflicting (multiple) keyAlias keys: '$existing' != '$key_alias'\n"
                    if $key_alias ne $existing;
                my $alias = $properties->{$field_name}->{alias} // '<missing alias>';
                my $existing_alias = $key_alias_info->{alias};
                die "conflicting alias for keyAlias: '$existing_alias' != '$alias'\n"
                    if $alias ne $existing_alias;
                push $key_alias_info->{values}->@*, $field_name;
            } else {
                my $alias = $properties->{$field_name}->{alias}
                    or die "missing 'alias' for 'keyAlias'-key\n";
                $key_alias_info = {
                    key_alias => $key_alias,
                    alias => $alias,
                    values => [$field_name],
                }
            }
            next;
        } elsif (exists $properties->{$field_name}->{alias}) {
            next;
        }
        for my $base (@array_bases) {
            next PROPERTY if $field_name =~ $base;
        }

        if (my ($base, $count) = is_array($properties, $field_name)) {
            push @array_bases, qr/^\Q$base\E\d+$/;

            my $field_rust_name = namify_field($base);
            my $original_field = $properties->{$field_name};
            my $field_schema = { $original_field->%* };
            $properties->{$field_name} = $field_schema;
            if (delete $field_schema->{default_key}) {
                die "default key points to array element '$field_name'\n";
            }
            my $field = make_struct_array_field(
                $name_hint,
                $base,
                $field_rust_name,
                $count,
                \$field_schema,
                $original_field,
            );
            die "duplicate field name '$field_name'\n" if exists($def->{fields}->{$field_name});
            if (exists($properties->{$base})) {
                warn "schema has flattened array as well as a field named '$base', '$field_name'...\n";
                $field->{rust_name}
                    = $field->{name}
                    = $field_name
                    = "${base}_array";
            } else {
                $field_name = $base;
            }
            $def->{fields}->{$field_name} = $field;
            $def->{api}->{properties}->{$field_name} = {
                type => $field->{array_type_name},
            };
        } else {
            my $field_rust_name = namify_field($field_name);
            my $field_schema = { $properties->{$field_name}->%* };
            $properties->{$field_name} = $field_schema;
            if (delete $field_schema->{default_key}) {
                $def->{api}->{default_key} = "\"$field_name\"";
            }
            my $field = make_struct_field($name_hint, $field_name, $field_rust_name, \$field_schema);
            die "duplicate field name '$field_name'\n" if exists($def->{fields}->{$field_name});
            $def->{fields}->{$field_name} = $field;
            $def->{api}->{properties}->{$field_name} = $field->{api};
        }
    }

    if (defined($key_alias_info)) {
        use Data::Dumper;
        my ($key_alias, $values, $alias) = $key_alias_info->@{qw(key_alias values alias)};
        $values = join ",\n            ", map { "\"$_\"" } @$values;
        my $api_key_alias_info = <<"EOF";
proxmox_schema::KeyAliasInfo::new(
        \"$key_alias\",
        &[
            $values
        ],
        \"$alias\"
    )
EOF
        chomp $api_key_alias_info;
        $def->{api}->{key_alias_info} = $api_key_alias_info;
    }

    my $additional = delete($schema->{additionalProperties}); # default is 1 urrrgh
    if (!defined($additional)) {
        # We don't know whether to just ignore it or actually scatter
        # `HashMap<String, serde_json::Value>` entries throughout the code.
        # But if we don't hit this too often the latter is probably fine.
        #
        # Default is 1 so warn...
        # If it's explicit we die (below).
        #warn "struct with arbitrary additional properties currently not supported\n";
    } elsif ($additional) {
        if (ref($additional)) {
            # FIXME: proxmox_schema doesn't support this yet.
            die "struct with additional properties not currently supported, don't know how to name the field\n";
        } else {
            $def->{api}->{additional_properties} = '"additional_properties"';
            $def->{additional_properties} = true;
            #die "struct with arbitrary additional properties currently not supported\n";
        }
    }

    warn_unused($schema, 'struct');

    return $name_hint;
}

my sub get_child_link_info : prototype($) {
    my ($schema) = @_;

    my $links = $schema->{links};
    return if !$links;

    die "unhandled links type in $\n" if @$links != 1;
    $links = $links->[0];
    die "unhandled links 'rel': $links->{rel}\n" if $links->{rel} ne 'child';
    my $href = $links->{href} or die "missing 'href' in 'links'\n";
    die if $href !~ /^\{(.*)\}$/;
    return $1;
}

sub method_return_array : prototype($$$) {
    my ($name_hint, $schema, $extra) = @_;
    die "no array name defined, unnamed return type?\n" if !defined($name_hint);
    local $__err_path = "$__err_path => array $name_hint";

    my $items = $schema->{items} // confess "no 'items' in array schema\n";

    # Special handling of links when the return struct has only a single element:
    if (defined(my $href = get_child_link_info($schema))) {
        my $properties = $items->{properties};
        if ($properties && 1 == keys($properties->%*)) {
            my $property = $properties->{$href}
                or die "link href '$href' does not exist in returned property\n";
            my $type = type_of($property);
            if ($type eq 'string') {
                return 'Vec<String>';
            } else {
                die "unhandled link type '$type' (in href '$href')\n";
            }
        }
    }

    my $def = {
        kind => 'array',
        name => $name_hint,
        items => undef,
        description => '',
        api => {},
        rust_name => $name_hint,
    };

    handle_def($def, \$items, $name_hint);

    return "Vec<$def->{type}>";
}

# Collect all path parameters present in an url.
# (The 'node' and 'vmid' in `/nodes/{node}/qemu/{vmid}/status`.)
my sub url_parameters : prototype($) {
    my ($path) = @_;
    my @params = ($path =~ /\{([^}]+)\}/g);
    return \@params;
}

# remove named params from the url
sub url_with_unnamed_params : prototype($) {
    my ($path) = @_;
    $path =~ s/\{([^}]+)\}/\{\}/g;
    return $path;
}

### Extract method parameters and deal with path based parameters.
# $api_method is the dumped schema's method definition.
my sub method_parameters : prototype($$$$$) {
    my ($def, $api_url, $param_name, $api_method, $rust_method_name) = @_;

    local $__list_format_as_array = 1;

    my $url_params = url_parameters($api_url);

    my $parameters = $api_method->{parameters} // {};

    #print "URL PARAMETERS: ".join(', ', $url_params->@*)."\n";

    # Clone to avoid modifying the duplicate.
    $parameters = { $parameters->%* };
    my $properties = { ($parameters->{properties} // {})->%* };
    $parameters->{properties} = $properties;

    # Handle URL parameter types. These should only be strings or integers.
    my $url_param_defs = [];
    for my $param ($url_params->@*) {
        my $schema = delete $properties->{$param};

        my $def = {
            kind => 'param',
            name => $param,
            rust_name => namify_field($param),
            type => undef,
            optional => bool(delete($schema->{optional})),
            description => '',
        };

        handle_def($def, \$schema, namify_type($rust_method_name, $param));

        # URL parameters use &str instead of String.
        if ($def->{type} eq 'String') {
            $def->{type} = '&str';
        }

        push @$url_param_defs, [$param, $def];
    }
    $def->{url_params} = $url_param_defs;


    if ($def->{http_method} eq 'GET' && keys($properties->%*) + $url_params->@* <= 6) {
        # GET methods should only have string parameters, and if there are <= 6 total we won't
        # build a struct.
        my $properties = { $properties->%* };
        my $input = [];
        my $handle = sub {
            my ($param, $schema) = @_;

            my $def = {
                kind => 'param',
                name => $param,
                rust_name => namify_field($param),
                type => undef,
                optional => bool(delete($schema->{optional})),
                description => '',
            };

            handle_def($def, \$schema, namify_type($rust_method_name, $param));
            if ($def->{optional}) {
                $def->{type} = "Option<$def->{type}>";
            }
            push @$input, $def;
        };
        #for my $param ($url_params->@*) {
        #    my $schema = { (delete($properties->{$param}))->%* };
        #    $handle->($param, $schema);
        #}
        for my $param (sort keys $properties->%*) {
            $handle->($param, $properties->{$param});
        }
        $def->{input} = $input;
        return;
    }

    return if !keys($parameters->%*);

    if (!$parameters->{additionalProperties}
        && (!$parameters->{properties} || !$parameters->{properties}->%*)
        && !scalar(grep { $_ ne 'additionalProperties' && $_ ne 'properties' } keys $parameters->%*))
    {
        # Sometimes we have empty objects as explicit parameters, avoid making types for them:
        return;
    }

    $def->{input_type} = generate_struct(
        $param_name,
        $parameters,
        {}, # { skip => $url_params },
        {},
    );
}

### Handle the return type...
#
my sub method_return_type : prototype($$$$) {
    my ($def, $method, $return_name, $extra) = @_;

    local $__list_format_as_array = 0;

    if (defined(my $returns = $extra->{'output-type'})) {
        $def->{output_type} = $returns;
        return;
    }

    my $returns = $extra->{'return-type'} // $method->{returns};

    my $type = type_of($returns);
    if ($type eq 'null') {
        $def->{output_type} = '()';
    } elsif ($type eq 'object') {
        $def->{output_type} = generate_struct(
            $return_name,
            $returns,
            {},
            {},
        );
    } elsif ($type eq 'array') {
        $def->{output_type} = method_return_array(
            $return_name,
            $returns,
            {},
        );
    } elsif ($type eq 'string') {
        my $schema = { $returns->%* };
        my $api = {};
        $def->{output_type} = string_type($schema, $api, $return_name, undef);
        die "unhandled return type api options: ".join(', ', sort keys $api->%*)."\n"
            if $api->%*;
    } else {
        die "unhandled return type: $type\n";
    }

    if ($extra->{attribs}) {
        $def->{output_type} = "ApiResponseData<$def->{output_type}>";
        $def->{'returns-attribs'} = 1;
    }
}

### Create an API method.
sub create_method : prototype($$$;%) {
    my (
        $api_url,
        $method,
        $rust_method_name,
        %extra,
    ) = @_;

    die "duplicate method name '$rust_method_name'\n"
        if exists($all_methods->{$rust_method_name});

    local $SIG{__DIE__} = sub {
        die "<$method->{method} ${api_url}>: $__err_path: $_[0]";
    };
    local $SIG{__WARN__} = sub {
        warn "<$method->{method} ${api_url}>: $__err_path: $_[0]";
    };

    my $param_name = $extra{'param-name'};
    my $return_name = $extra{'return-name'};
    if (!defined($return_name) && defined($param_name)) {
        $return_name = "${param_name}Response";
    }

    my $def = {
        kind => 'method',
        url => $api_url,
        name => $rust_method_name,
        http_method => $method->{method},
        description => $method->{description},
    };

    method_parameters($def, $api_url, $param_name, $method, $rust_method_name);
    method_return_type($def, $method, $return_name, \%extra);
    $all_methods->{$rust_method_name} = $def;
}

sub add_custom_method : prototype($$) {
    my ($rust_method_name, $def) = @_;
    $all_methods->{$rust_method_name} = $def;
}

##################################
#
# Helpers to walk the API method paths

# Maps path nodes in the api tree to their path text.
# (used to show for which methods no wrapper was generated)
my $ALL_PATHS = {};
my $API_ROOT;

# Get a hash of all the unused paths.
sub get_unused_paths() {
    return $ALL_PATHS;
}

my sub walk_api_ : prototype($$$);
sub walk_api_ : prototype($$$) {
    my ($node, $method, $path) = @_;

    if (!length($path)) {
        return (
            $node->{path},
            $node->{info}->{$method} // die "no such method '$method'\n",
        );
    }

    my ($current, $rest) = ($path =~ m!^/*([^/]+)(/.*)?$!);

    for my $entry ($node->{children}->@*) {
        if ($entry->{text} eq $current) {
            return walk_api_($entry, $method, $rest // '');
        }
    }
    die "not found\n";
}

my sub walk_api : prototype($$) {
    my ($method, $path) = @_;
    local $SIG{__DIE__} = sub { die "failed to query path '$path': $_[0]"; };
    return walk_api_({ children => $API_ROOT }, $method, $path);
}

# Create wrappers for an API method.
sub api : prototype($$$;%) {
    my ($method, $api_url, $rust_method_name, %extra) = @_;
    delete $ALL_PATHS->{$api_url};

    (my $path, $method) = walk_api($method, $api_url);
    croak "missing method for '$api_url'\n" if !defined($method);
    create_method($api_url, $method, $rust_method_name, %extra);
}

# Fills $ALL_PATHS recursively from an api node.
my sub collect_paths : prototype($$);
sub collect_paths : prototype($$) {
    my ($node, $path) = @_;
    $ALL_PATHS->{$path} = 1 if length($path);
    my $children = $node->{children};
    return if !$children;

    for my $c ($children->@*) {
        collect_paths($c, "$path/$c->{text}");
    }
}

# Initialize access to an API. Should be a PVE or PMG API root.
# Also sets up the `__DIE__` and `__WARN__` signals to include context.
sub init_api : prototype($$) ($root, $lookup_format) {
    $LOOKUP_FORMAT = $lookup_format;
    $API_ROOT = $root;
    collect_paths({ children => $API_ROOT }, '');

    $SIG{__DIE__} = sub { die "$Schema2Rust::__err_path: $_[0]" };
    $SIG{__WARN__} = sub { warn "$Schema2Rust::__err_path: $_[0]" };
}

sub debug_path : prototype($$) {
    my ($method, $path) = @_;

    use Data::Dumper;

    $method = walk_api($method, $path);
    print(Dumper($method));
}

1;
