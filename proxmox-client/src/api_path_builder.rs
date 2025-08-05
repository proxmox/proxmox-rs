use std::borrow::Cow;

/// Builder for API paths with a query.
///
/// The [`arg`](Self::arg) method can be used to add multiple arguments to the query.
///
/// ```rust
/// use proxmox_client::ApiPathBuilder;
///
/// let storage = "my-storage";
/// let target = "my-target";
/// let node = "pve01";
/// let query = ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/storage"))
///     .arg("storage", storage)
///     .arg("target", target)
///     .build();
///
/// assert_eq!(&query, "/api2/extjs/nodes/pve01/storage?storage=my%2Dstorage&target=my%2Dtarget");
/// ```
///
/// ## Compatibility with perl HTTP servers
///
/// The following methods are added for compatibility with perl HTTP servers.
///
/// - `bool_arg`: translates booleans to `"0"`/`"1"`
/// - `list_arg`: split lists so that they can be feed to perl's `split_list()`
///
/// These methods require the `perl-api-path-builder` feature.
#[derive(Clone, Debug)]
pub struct ApiPathBuilder {
    url: String,
    separator: char,
}

impl ApiPathBuilder {
    /// Creates a new builder from a base path.
    pub fn new<'a>(base: impl Into<Cow<'a, str>>) -> Self {
        Self {
            url: base.into().into_owned(),
            separator: '?',
        }
    }

    /// Adds an argument to the query.
    ///
    /// The name and value will be percent-encoded.
    pub fn arg<T: std::fmt::Display>(mut self, name: &str, value: T) -> Self {
        self.push_separator_and_name(name);
        self.push_encoded(value.to_string().as_bytes());
        self
    }

    /// Adds an optional argument to the query.
    ///
    /// Does nothing if the value is `None`. See [`arg`](Self::arg) for more details.
    pub fn maybe_arg<T: std::fmt::Display>(mut self, name: &str, value: &Option<T>) -> Self {
        if let Some(value) = value {
            self = self.arg(name, value);
        }
        self
    }

    /// Builds the url.
    pub fn build(self) -> String {
        self.url
    }

    fn push_separator_and_name(&mut self, name: &str) {
        self.url.push(self.separator);
        self.separator = '&';
        self.push_encoded(name.as_bytes());
        self.url.push('=');
    }

    fn push_encoded(&mut self, value: &[u8]) {
        let enc_value = percent_encoding::percent_encode(value, percent_encoding::NON_ALPHANUMERIC);
        self.url.extend(enc_value);
    }
}

#[cfg(feature = "perl-api-path-builder")]
impl ApiPathBuilder {
    /// Adds a boolean arg in a perl-friendly fashion.
    ///
    /// ```rust
    /// use proxmox_client::ApiPathBuilder;
    ///
    /// let node = "pve01";
    /// let query = ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/storage"))
    ///     .bool_arg("enabled", true)
    ///     .build();
    ///
    /// assert_eq!(&query, "/api2/extjs/nodes/pve01/storage?enabled=1");
    /// ```
    ///
    /// `true` will be converted into `"1"` and `false` to `"0"`.
    pub fn bool_arg(mut self, name: &str, value: bool) -> Self {
        self.push_separator_and_name(name);
        if value {
            self.url.push('1');
        } else {
            self.url.push('0');
        };
        self
    }

    /// Adds an optional boolean arg in a perl-friendly fashion.
    ///
    /// Does nothing if `value` is `None`. See [`bool_arg`](Self::bool_arg) for more
    /// details.
    pub fn maybe_bool_arg(mut self, name: &str, value: Option<bool>) -> Self {
        if let Some(value) = value {
            self = self.bool_arg(name, value);
        }
        self
    }

    /// Helper for building perl-friendly queries.
    ///
    /// For `<type>-list` entries we turn an array into a string ready for
    /// perl's `split_list()` call.
    ///
    /// ```rust
    /// use proxmox_client::ApiPathBuilder;
    ///
    /// let content = vec!["backup", "images"];
    /// let node = "my_node";
    /// let query = proxmox_client::ApiPathBuilder::new(format!("/api2/extjs/nodes/{node}/storage"))
    ///     .list_arg("content", &content)
    ///     .arg("type", "vm")
    ///     .build();
    ///
    /// assert_eq!(&query, "/api2/extjs/nodes/my_node/storage?content=backup%00images&type=vm");
    /// ```
    ///
    /// The name and values will be percent-encoded.
    pub fn list_arg<I>(mut self, name: &str, values: I) -> Self
    where
        I: IntoIterator<Item: std::fmt::Display>,
    {
        self.push_separator_and_name(name);
        let mut list_separator = "";
        for entry in values.into_iter() {
            self.url.push_str(list_separator);
            list_separator = "%00";
            self.push_encoded(entry.to_string().as_bytes());
        }
        self
    }

    /// Helper for building perl-friendly queries.
    ///
    /// See [`list_arg`](Self::list_arg) for more details.
    pub fn maybe_list_arg<T: std::fmt::Display>(
        mut self,
        name: &str,
        values: &Option<Vec<T>>,
    ) -> Self {
        if let Some(values) = values {
            self = self.list_arg(name, values.iter());
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // See pdm_api_types::ConfigurationState.
    #[derive(serde::Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum ConfigurationState {
        Active,
    }
    serde_plain::derive_display_from_serialize!(ConfigurationState);

    // See pve_api_types::ClusterResourceKind.
    #[derive(serde::Serialize)]
    enum ClusterResourceKind {
        #[serde(rename = "vm")]
        Vm,
    }
    serde_plain::derive_display_from_serialize!(ClusterResourceKind);

    #[test]
    fn test_builder() {
        let expected = "/api2/extjs/cluster/resources?type=vm";
        let ty = ClusterResourceKind::Vm;

        let query = ApiPathBuilder::new("/api2/extjs/cluster/resources")
            .arg("type", ty)
            .build();

        assert_eq!(&query, expected);

        let second_expected =
            "/api2/extjs/pve/remotes/some-remote/qemu/100/config?state=active&node=myNode";
        let state = ConfigurationState::Active;
        let node = "myNode";
        let snapshot = None::<&str>;

        let second_query =
            ApiPathBuilder::new("/api2/extjs/pve/remotes/some-remote/qemu/100/config")
                .arg("state", state)
                .arg("node", node)
                .maybe_arg("snapshot", &snapshot)
                .build();

        assert_eq!(&second_query, &second_expected);
    }
}

#[cfg(all(test, feature = "perl-api-path-builder"))]
mod perl_tests {
    use super::*;

    #[test]
    fn test_perl_builder() {
        let history = true;
        let local_only = false;
        let start_time = 1000;

        let expected_url =
            "/api2/extjs/cluster/metrics/export?history=1&local%2Donly=0&start%2Dtime=1000";

        let query = ApiPathBuilder::new("/api2/extjs/cluster/metrics/export")
            .bool_arg("history", history)
            .bool_arg("local-only", local_only)
            .arg("start-time", start_time)
            .build();
        assert_eq!(expected_url, query);

        let query_with_maybe = ApiPathBuilder::new("/api2/extjs/cluster/metrics/export")
            .maybe_bool_arg("history", Some(history))
            .maybe_bool_arg("local-only", Some(local_only))
            .maybe_arg("start-time", &Some(start_time))
            .build();

        assert_eq!(expected_url, query_with_maybe);
    }
}
