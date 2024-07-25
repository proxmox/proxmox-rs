mod compression;
mod decompression;

pub use compression::{DeflateEncoder, DeflateEncoderBuilder, Level};
pub use decompression::{DeflateDecoder, DeflateDecoderBuilder};

const BUFFER_SIZE: usize = 8192;

#[cfg(test)]
mod test {
    use super::*;

    use std::io::Write;

    use flate2::Compression;
    use futures::StreamExt;

    const BUFFER_SIZE: usize = 25;
    const BODY: &str = r#"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
eiusmod tempor incididunt ut labore et dolore magnam aliquam quaerat voluptatem. Ut
enim aeque doleamus animo, cum corpore dolemus, fieri tamen permagna accessio potest,
si aliquod aeternum et infinitum impendere."#;

    fn chunker(content: &[u8]) -> Vec<Result<Vec<u8>, std::io::Error>> {
        vec![
            Ok(content[..10].to_vec()),
            Ok(content[10..20].to_vec()),
            Ok(content[20..30].to_vec()),
            Ok(content[30..40].to_vec()),
            Ok(content[40..].to_vec()),
        ]
    }

    #[tokio::test]
    async fn test_encoder_against_decoder() {
        // We use mixed sizes for the buffers, on the next test we invert the
        // sizes.
        let stream = futures::stream::iter(chunker(BODY.as_bytes()));
        let encoder = DeflateEncoder::builder(stream)
            .buffer_size(BUFFER_SIZE * 2)
            .build();
        let mut decoder = DeflateDecoder::builder(encoder)
            .buffer_size(BUFFER_SIZE)
            .build();

        let mut buf = Vec::with_capacity(BODY.len());
        while let Some(Ok(res)) = decoder.next().await {
            buf.write_all(&res).unwrap();
        }

        assert_eq!(buf, BODY.as_bytes());
    }

    #[tokio::test]
    async fn test_zlib_encoder_against_decoder() {
        let stream = futures::stream::iter(chunker(BODY.as_bytes()));
        let encoder = DeflateEncoder::builder(stream)
            .zlib(true)
            .buffer_size(BUFFER_SIZE)
            .build();
        let mut decoder = DeflateDecoder::builder(encoder)
            .zlib(true)
            .buffer_size(BUFFER_SIZE * 2)
            .build();

        let mut buf = Vec::with_capacity(BODY.len());
        while let Some(Ok(res)) = decoder.next().await {
            buf.write_all(&res).unwrap();
        }

        assert_eq!(buf, BODY.as_bytes());
    }

    #[tokio::test]
    async fn test_deflate_decompression_against_flate2() {
        let encoded = flate2_encode(BODY.as_bytes(), false).unwrap();
        let decoded = decode(&encoded, false, 7).await.unwrap();

        assert_eq!(decoded, BODY.as_bytes());
    }

    #[tokio::test]
    async fn test_zlib_decompression_against_flate2() {
        let encoded = flate2_encode(BODY.as_bytes(), true).unwrap();
        let decoded = decode(&encoded, true, 4).await.unwrap();

        assert_eq!(decoded, BODY.as_bytes());
    }

    #[tokio::test]
    async fn test_deflate_compression_against_flate2() {
        let encoded = encode(BODY.as_bytes(), false, 5).await.unwrap();
        let decoded = flate2_decode(&encoded, false).unwrap();

        assert_eq!(decoded, BODY.as_bytes());
    }

    #[tokio::test]
    async fn test_zlib_compression_against_flate2() {
        let encoded = encode(BODY.as_bytes(), true, 3).await.unwrap();
        let decoded = flate2_decode(&encoded, true).unwrap();

        assert_eq!(decoded, BODY.as_bytes());
    }

    fn flate2_encode(bytes: &[u8], is_zlib: bool) -> Result<Vec<u8>, std::io::Error> {
        if is_zlib {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), Compression::default());
            e.write_all(bytes).unwrap();
            e.finish()
        } else {
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(bytes).unwrap();
            e.finish()
        }
    }

    fn flate2_decode(bytes: &[u8], is_zlib: bool) -> Result<Vec<u8>, std::io::Error> {
        if is_zlib {
            let mut e = flate2::write::ZlibDecoder::new(Vec::new());
            e.write_all(bytes).unwrap();
            e.finish()
        } else {
            let mut e = flate2::write::DeflateDecoder::new(Vec::new());
            e.write_all(bytes).unwrap();
            e.finish()
        }
    }

    async fn decode(
        content: &[u8],
        is_zlib: bool,
        buffer_size: usize,
    ) -> Result<Vec<u8>, std::io::Error> {
        let stream = futures::stream::iter(chunker(content));
        let mut decoder = DeflateDecoder::builder(stream)
            .zlib(is_zlib)
            .buffer_size(buffer_size)
            .build();
        let mut buf = Vec::new();

        while let Some(Ok(res)) = decoder.next().await {
            buf.write_all(&res)?;
        }

        Ok(buf)
    }

    async fn encode(
        content: &[u8],
        is_zlib: bool,
        buffer_size: usize,
    ) -> Result<Vec<u8>, std::io::Error> {
        let stream = futures::stream::iter(chunker(content));
        let mut encoder = DeflateEncoder::builder(stream)
            .zlib(is_zlib)
            .buffer_size(buffer_size)
            .build();
        let mut buf = Vec::with_capacity(BODY.len());

        while let Some(Ok(res)) = encoder.next().await {
            buf.write_all(&res)?;
        }

        Ok(buf)
    }
}
