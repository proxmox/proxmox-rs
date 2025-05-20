macro_rules! implement_kind {
    (
        $alphabet:expr,
        #[$kind:meta]
        #[$doc_use_display:meta]
        #[$doc_display_formatted:meta]
        #[$doc_use_display_no_pad:meta]
        #[$doc_display_no_pad_formatted:meta]
        #[$doc_serialize:meta]
        #[$doc_serialize_json:meta]
        #[$doc_serialize_no_pad:meta]
        #[$doc_serialize_no_pad_json:meta]
        #[$doc_deserialize:meta]
        #[$doc_deserialize_pad:meta]
        #[$doc_deserialize_no_pad:meta]
        #[$doc_serde_with:meta]
        #[$doc_serde_with_no_pad_indifferent:meta]
        #[$doc_serde_with_must_pad:meta]
        #[$doc_serde_with_must_not_pad:meta]
    ) => {
        use std::fmt;

        use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
        use base64::engine::DecodePaddingMode;
        use base64::engine::Engine as _;

        #[cfg(feature = "serde")]
        use serde::{Deserialize, Deserializer, Serializer};
        #[cfg(feature = "serde")]
        use std::borrow::Cow;

        const ENGINE_MUST_PAD: GeneralPurpose = GeneralPurpose::new(
            $alphabet,
            GeneralPurposeConfig::new()
                .with_encode_padding(true)
                .with_decode_padding_mode(DecodePaddingMode::RequireCanonical),
        );

        const ENGINE_MUST_NOT_PAD: GeneralPurpose = GeneralPurpose::new(
            $alphabet,
            GeneralPurposeConfig::new()
                .with_encode_padding(false)
                .with_decode_padding_mode(DecodePaddingMode::RequireNone),
        );

        /// Must only be used for *de*coding.
        const DECODE_ENGINE_INDIFFERENT_PAD: GeneralPurpose = GeneralPurpose::new(
            $alphabet,
            GeneralPurposeConfig::new()
                .with_encode_padding(false)
                .with_decode_padding_mode(DecodePaddingMode::Indifferent),
        );

        /// Encode data as
        #[$kind]
        /// with padding.
        pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
            ENGINE_MUST_PAD.encode(data)
        }

        /// Encode data as
        #[$kind]
        /// without padding.
        pub fn encode_no_pad<T: AsRef<[u8]>>(data: T) -> String {
            ENGINE_MUST_NOT_PAD.encode(data)
        }

        /// Decode
        #[$kind]
        /// data with *optional* padding.
        pub fn decode<T: AsRef<[u8]>>(data: T) -> Result<Vec<u8>, DecodeError> {
            DECODE_ENGINE_INDIFFERENT_PAD.decode(data).convert_error()
        }

        /// Decode
        #[$kind]
        /// data which *must* be padded.
        pub fn decode_pad<T: AsRef<[u8]>>(data: T) -> Result<Vec<u8>, DecodeError> {
            ENGINE_MUST_PAD.decode(data).convert_error()
        }

        /// Decode
        #[$kind]
        /// data which *must not* be padded.
        pub fn decode_no_pad<T: AsRef<[u8]>>(data: T) -> Result<Vec<u8>, DecodeError> {
            ENGINE_MUST_NOT_PAD.decode(data).convert_error()
        }

        /*
        /// Encode data as
        #[$kind]
        /// with padding into a slice.
        pub fn encode_slice<T>(data: T, output: &mut [u8]) -> Result<usize, EncodeError>
        where
            T: AsRef<[u8]>,
        {
            ENGINE_MUST_PAD.encode_slice(data, output).convert_error()
        }

        /// Encode data as
        #[$kind]
        /// without padding into a slice.
        pub fn encode_slice_no_pad<T>(data: T, output: &mut [u8]) -> Result<usize, EncodeError>
        where
            T: AsRef<[u8]>,
        {
            ENGINE_MUST_NOT_PAD
                .encode_slice(data, output)
                .convert_error()
        }

        /// Decode
        #[$kind]
        /// data with *optional* padding into a slice.
        pub fn decode_slice<T>(data: T, output: &mut [u8]) -> Result<usize, DecodeError>
        where
            T: AsRef<[u8]>,
        {
            DECODE_ENGINE_INDIFFERENT_PAD
                .decode_slice(data, output)
                .convert_error()
        }

        /// Decode
        #[$kind]
        /// data which *must* be padded into a slice.
        pub fn decode_slice_pad<T>(data: T, output: &mut [u8]) -> Result<usize, DecodeError>
        where
            T: AsRef<[u8]>,
        {
            ENGINE_MUST_PAD.decode_slice(data, output).convert_error()
        }

        /// Decode
        #[$kind]
        /// data which *must not* be padded into a slice.
        pub fn decode_slice_no_pad<T>(data: T, output: &mut [u8]) -> Result<usize, DecodeError>
        where
            T: AsRef<[u8]>,
        {
            ENGINE_MUST_NOT_PAD
                .decode_slice(data, output)
                .convert_error()
        }
        */

        /// A formatting wrapper producing
        #[$kind]
        /// data with padding.
        ///
        /// Usage example:
        /// ```
        #[$doc_use_display]
        ///
        /// let message = "some text";
        /// let data = b"1~~2";
        /// assert_eq!(
        ///     format!("{}", Display(&data)),
        #[$doc_display_formatted]
        /// );
        /// ```
        pub struct Display<T: AsRef<[u8]>>(pub T);

        impl<T: AsRef<[u8]>> fmt::Display for Display<T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(
                    &base64::display::Base64Display::new(self.0.as_ref(), &ENGINE_MUST_PAD),
                    f,
                )
            }
        }
        ///
        /// A formatting wrapper producing
        #[$kind]
        /// data without padding.
        ///
        /// Usage example:
        /// ```
        #[$doc_use_display_no_pad]
        ///
        /// let message = "some text";
        /// let data = b"1~~2";
        /// assert_eq!(
        ///     format!("{}", DisplayNoPad(&data)),
        #[$doc_display_no_pad_formatted]
        /// );
        /// ```
        pub struct DisplayNoPad<T: AsRef<[u8]>>(pub T);

        impl<T: AsRef<[u8]>> fmt::Display for DisplayNoPad<T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(
                    &base64::display::Base64Display::new(self.0.as_ref(), &ENGINE_MUST_NOT_PAD),
                    f,
                )
            }
        }

        /// Serialize bytes as
        #[$kind]
        /// encoded string with padding.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Serialize)]
        /// struct Foo {
        #[$doc_serialize]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_json]
        /// assert_eq!(json, encoded);
        /// ```
        #[cfg(feature = "serde")]
        pub fn serialize_as_base64<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            T: AsRef<[u8]>,
            S: Serializer,
        {
            serializer.serialize_str(&encode(data))
        }

        /// Serialize bytes as
        #[$kind]
        /// encoded string without padding.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Serialize)]
        /// struct Foo {
        #[$doc_serialize_no_pad]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_no_pad_json]
        /// assert_eq!(json, encoded);
        /// ```
        #[cfg(feature = "serde")]
        pub fn serialize_as_base64_no_pad<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            T: AsRef<[u8]>,
            S: Serializer,
        {
            serializer.serialize_str(&encode_no_pad(data))
        }

        /// Deserialize from a
        #[$kind]
        /// encoded string with *optional* padding.
        ///
        /// Usage example:
        /// ```
        /// # use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq)]
        /// struct Foo {
        #[$doc_deserialize]
        ///     data: Vec<u8>,
        /// }
        ///
        #[$doc_serialize_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        /// // padding is optional:
        #[$doc_serialize_no_pad_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        /// ```
        #[cfg(feature = "serde")]
        pub fn deserialize_from_base64<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
            T: From<Vec<u8>>,
        {
            let data = Cow::<str>::deserialize(deserializer)?;
            let data = decode(data.as_bytes()).map_err(::serde::de::Error::custom)?;
            Ok(T::from(data))
        }

        /// Deserialize from a
        #[$kind]
        /// encoded string which *must* be padded.
        ///
        /// Usage example:
        /// ```
        /// # use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq)]
        /// struct Foo {
        #[$doc_deserialize_pad]
        ///     data: Vec<u8>,
        /// }
        ///
        #[$doc_serialize_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        #[$doc_serialize_no_pad_json]
        /// serde_json::from_str::<Foo>(&encoded).expect_err("expected to fail decoding unpadded data");
        /// ```
        #[cfg(feature = "serde")]
        pub fn deserialize_from_base64_pad<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
            T: From<Vec<u8>>,
        {
            let data = Cow::<str>::deserialize(deserializer)?;
            let data = decode_pad(data.as_bytes()).map_err(serde::de::Error::custom)?;
            Ok(T::from(data))
        }

        /// Deserialize from a
        #[$kind]
        /// encoded string which *must not* be padded.
        ///
        /// Usage example:
        /// ```
        /// # use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq)]
        /// struct Foo {
        #[$doc_deserialize_no_pad]
        ///     data: Vec<u8>,
        /// }
        ///
        #[$doc_serialize_no_pad_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        #[$doc_serialize_json]
        /// serde_json::from_str::<Foo>(&encoded).expect_err("expected to fail decoding padded data");
        /// ```
        #[cfg(feature = "serde")]
        pub fn deserialize_from_base64_no_pad<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
            T: From<Vec<u8>>,
        {
            use serde::de::Error;
            let data = Cow::<str>::deserialize(deserializer)?;
            let data =
                decode_no_pad(data.as_bytes()).map_err(|err| Error::custom(err.to_string()))?;
            Ok(T::from(data))
        }

        /// Serialize and deserialize from a
        #[$kind]
        /// encoded string.
        /// The output will be padded, the input *may* be padded.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq, Serialize)]
        /// struct Foo {
        #[$doc_serde_with]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_json]
        /// assert_eq!(json, encoded);
        ///
        #[$doc_serialize_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        /// // the padding is optional:
        #[$doc_serialize_no_pad_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        /// ```
        #[cfg(feature = "serde")]
        pub mod as_base64 {
            pub use super::deserialize_from_base64 as deserialize;
            pub use super::serialize_as_base64 as serialize;
        }

        /// Serialize and deserialize from a
        #[$kind]
        /// encoded string.
        /// The output will *not* be padded, the input *may* be padded.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq, Serialize)]
        /// struct Foo {
        #[$doc_serde_with_no_pad_indifferent]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_no_pad_json]
        /// assert_eq!(json, encoded);
        ///
        #[$doc_serialize_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        /// // the padding is optional:
        #[$doc_serialize_no_pad_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        /// ```
        #[cfg(feature = "serde")]
        pub mod as_base64_no_pad_indifferent {
            pub use super::deserialize_from_base64 as deserialize;
            pub use super::serialize_as_base64_no_pad as serialize;
        }

        /// Serialize and deserialize from a
        #[$kind]
        /// encoded string.
        /// The output will be padded, the input *must* be padded.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq, Serialize)]
        /// struct Foo {
        #[$doc_serde_with_must_pad]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_json]
        /// assert_eq!(json, encoded);
        ///
        #[$doc_serialize_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        #[$doc_serialize_no_pad_json]
        /// serde_json::from_str::<Foo>(&encoded).expect_err("expected to fail decoding unpadded data");
        /// ```
        #[cfg(feature = "serde")]
        pub mod as_base64_must_pad {
            pub use super::deserialize_from_base64_pad as deserialize;
            pub use super::serialize_as_base64 as serialize;
        }

        /// Serialize and deserialize from a
        #[$kind]
        /// encoded string.
        /// The output will *not* be padded, the input *must not* be padded.
        ///
        /// Usage example:
        /// ```
        /// use serde::{Deserialize, Serialize};
        ///
        /// # #[derive(Debug)]
        /// #[derive(Deserialize, PartialEq, Serialize)]
        /// struct Foo {
        #[$doc_serde_with_must_not_pad]
        ///     data: Vec<u8>,
        /// }
        ///
        /// let obj = Foo { data: b"1~~2".into() };
        /// let json = serde_json::to_string(&obj).unwrap();
        #[$doc_serialize_no_pad_json]
        /// assert_eq!(json, encoded);
        ///
        #[$doc_serialize_no_pad_json]
        /// let deserialized: Foo = serde_json::from_str(&encoded).unwrap();
        /// assert_eq!(deserialized, Foo { data: b"1~~2".into() });
        ///
        #[$doc_serialize_json]
        /// serde_json::from_str::<Foo>(&encoded).expect_err("expected to fail decoding unpadded data");
        /// ```
        #[cfg(feature = "serde")]
        pub mod as_base64_must_not_pad {
            pub use super::deserialize_from_base64_no_pad as deserialize;
            pub use super::serialize_as_base64_no_pad as serialize;
        }
    };
}
