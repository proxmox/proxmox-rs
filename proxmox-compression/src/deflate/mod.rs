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

    /// Decode an unfinished deflate stream, as a receiver does while the sender keeps writing.
    fn flate2_decode_partial(bytes: &[u8], is_zlib: bool) -> Vec<u8> {
        let mut decoder = flate2::Decompress::new(is_zlib);
        let mut out = Vec::with_capacity(bytes.len() * 4 + 64);
        decoder
            .decompress_vec(bytes, &mut out, flate2::FlushDecompress::Sync)
            .unwrap();
        out
    }

    /// With a flush per chunk, the output for a chunk is yielded as soon as the chunk was read and
    /// decodes to the input so far, while the sender keeps the stream open.
    #[tokio::test]
    async fn test_flush_per_chunk_yields_each_chunk_decodable() {
        use futures::FutureExt;

        for is_zlib in [false, true] {
            let (sender, receiver) = futures::channel::mpsc::unbounded();
            let mut encoder = DeflateEncoder::builder(receiver)
                .zlib(is_zlib)
                .buffer_size(BUFFER_SIZE)
                .flush_per_chunk(true)
                .build();
            let mut encoded = Vec::new();
            let mut expected = Vec::new();
            for chunk in chunker(BODY.as_bytes()) {
                let chunk = chunk.unwrap();
                expected.extend_from_slice(&chunk);
                sender.unbounded_send(Ok(chunk)).unwrap();
                let mut yielded = 0;
                while let Some(item) = encoder.next().now_or_never() {
                    encoded.extend_from_slice(&item.unwrap().unwrap());
                    yielded += 1;
                }
                assert!(yielded > 0, "no output after a chunk");
                assert_eq!(flate2_decode_partial(&encoded, is_zlib), expected);
            }

            // Without the flush, the first chunks stay inside the compressor.
            let (sender, receiver) = futures::channel::mpsc::unbounded();
            let mut encoder = DeflateEncoder::builder(receiver)
                .zlib(is_zlib)
                .buffer_size(BUFFER_SIZE)
                .build();
            sender
                .unbounded_send(Ok(BODY.as_bytes()[..10].to_vec()))
                .unwrap();
            assert!(encoder.next().now_or_never().is_none());
        }
    }

    /// A flush must complete with any output buffer size: the compressor has to be asked for it
    /// once and drained afterwards, as asking again after the buffer ran full writes another flush
    /// marker every time and never ends.
    #[tokio::test]
    async fn test_flushing_terminates_with_tiny_buffers() {
        use futures::FutureExt;

        for buffer_size in 1..=8 {
            for per_chunk in [false, true] {
                let stream = futures::stream::iter(chunker(BODY.as_bytes()));
                let mut encoder = DeflateEncoder::builder(stream)
                    .buffer_size(buffer_size)
                    .flush_window((!per_chunk).then_some(1))
                    .flush_per_chunk(per_chunk)
                    .build();
                let mut encoded = Vec::new();
                let mut polls = 0;
                loop {
                    polls += 1;
                    assert!(
                        polls < 10_000,
                        "buffer {buffer_size}: flushing does not end"
                    );
                    match encoder.next().now_or_never() {
                        Some(Some(item)) => encoded.extend_from_slice(&item.unwrap()),
                        Some(None) => break,
                        None => panic!("the input never pends"),
                    }
                }
                assert_eq!(flate2_decode(&encoded, false).unwrap(), BODY.as_bytes());
            }
        }
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
