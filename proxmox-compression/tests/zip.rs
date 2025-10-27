use std::io::Cursor;

use anyhow::{ensure, Result};
use flate2::{Decompress, FlushDecompress};
use tokio::test;

use proxmox_compression::zip::{FileType, ZipEncoder, ZipEntry};

fn check_zip_with_one_file(
    zip_file: &[u8],
    expected_file_name: &str,
    expected_file_attributes: u16,
    expected_content: Option<&[u8]>,
) -> Result<()> {
    ensure!(zip_file.starts_with(b"PK\x03\x04"));

    let general_purpose_flags = &zip_file[6..8];
    let size_compressed = &zip_file[18..22];
    let size_uncompressed = &zip_file[22..26];
    let file_name_len = (zip_file[26] as usize) | ((zip_file[27] as usize) << 8);
    let extra_len = zip_file[28] as usize | ((zip_file[29] as usize) << 8);
    let file_name = &zip_file[30..30 + file_name_len];
    let mut offset = 30 + file_name_len;

    ensure!(file_name == expected_file_name.as_bytes());

    offset += extra_len;

    if let Some(expected_content) = expected_content {
        let mut decompress = Decompress::new(false);
        let mut decompressed = Vec::with_capacity(expected_content.len());
        decompress.decompress_vec(
            &zip_file[offset..],
            &mut decompressed,
            FlushDecompress::Finish,
        )?;

        ensure!(decompressed == expected_content);

        offset += decompress.total_in() as usize;
    }

    // Optional data descriptor
    if &zip_file[offset..offset + 4] == b"PK\x07\x08" {
        offset += 4;

        if (general_purpose_flags[0] & 8) != 0 {
            offset += 12;

            if size_compressed == b"\xff\xff\xff\xff" && size_uncompressed == b"\xff\xff\xff\xff" {
                offset += 8;
            }
        }
    }

    ensure!(
        &zip_file[offset..offset + 4] == b"PK\x01\x02",
        "Expecting a central directory file header"
    );

    let external_file_attributes = &zip_file[offset + 38..offset + 42];
    let file_attributes = u16::from_le_bytes(external_file_attributes[2..4].try_into()?);

    ensure!(file_attributes == expected_file_attributes);

    Ok(())
}

#[test]
async fn test_zip_file() -> Result<()> {
    let mut zip_file = Vec::new();
    let mut zip_encoder = ZipEncoder::new(&mut zip_file);
    zip_encoder
        .add_entry(
            ZipEntry::new("foo", 0, 0o755, FileType::Regular),
            Some(Cursor::new(b"bar")),
        )
        .await?;
    zip_encoder.finish().await?;

    check_zip_with_one_file(&zip_file, "foo", 0o100755, Some(b"bar"))?;

    Ok(())
}

#[test]
async fn test_zip_symlink() -> Result<()> {
    let mut zip_file = Vec::new();
    let mut zip_encoder = ZipEncoder::new(&mut zip_file);
    zip_encoder
        .add_entry(
            ZipEntry::new("link", 0, 0o755, FileType::Symlink),
            Some(Cursor::new(b"/dev/null")),
        )
        .await?;
    zip_encoder.finish().await?;

    check_zip_with_one_file(&zip_file, "link", 0o120755, Some(b"/dev/null"))?;

    Ok(())
}

#[test]
async fn test_zip_directory() -> Result<()> {
    let mut zip_file = Vec::new();
    let mut zip_encoder = ZipEncoder::new(&mut zip_file);
    zip_encoder
        .add_entry::<&[u8]>(
            ZipEntry::new("directory", 0, 0o755, FileType::Directory),
            None,
        )
        .await?;
    zip_encoder.finish().await?;

    check_zip_with_one_file(&zip_file, "directory/", 0o40755, None)?;

    Ok(())
}
