//! Test the automatic addition of integer limits.

use proxmox_schema::ApiType;
use proxmox_api_macro::api;

/// An i16: -32768 to 32767.
#[api]
pub struct AnI16(i16);

#[test]
fn test_an_i16_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("An i16: -32768 to 32767.")
            .minimum(-32768)
            .maximum(32767)
            .schema();

    assert_eq!(TEST_SCHEMA, AnI16::API_SCHEMA);
}

/// Already limited on one side.
#[api(minimum: -50)]
pub struct I16G50(i16);

#[test]
fn test_i16g50_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("Already limited on one side.")
            .minimum(-50)
            .maximum(32767)
            .schema();

    assert_eq!(TEST_SCHEMA, I16G50::API_SCHEMA);
}

/// An i32: -0x8000_0000 to 0x7fff_ffff.
#[api]
pub struct AnI32(i32);

#[test]
fn test_an_i32_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("An i32: -0x8000_0000 to 0x7fff_ffff.")
            .minimum(-0x8000_0000)
            .maximum(0x7fff_ffff)
            .schema();

    assert_eq!(TEST_SCHEMA, AnI32::API_SCHEMA);
}

/// Unsigned implies a minimum of zero.
#[api]
pub struct AnU32(u32);

#[test]
fn test_an_u32_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("Unsigned implies a minimum of zero.")
            .minimum(0)
            .maximum(0xffff_ffff)
            .schema();

    assert_eq!(TEST_SCHEMA, AnU32::API_SCHEMA);
}

/// An i64: this is left unlimited.
#[api]
pub struct AnI64(i64);

#[test]
fn test_an_i64_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("An i64: this is left unlimited.").schema();

    assert_eq!(TEST_SCHEMA, AnI64::API_SCHEMA);
}

/// Unsigned implies a minimum of zero.
#[api]
pub struct AnU64(u64);

#[test]
fn test_an_u64_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::IntegerSchema::new("Unsigned implies a minimum of zero.")
            .minimum(0)
            .schema();

    assert_eq!(TEST_SCHEMA, AnU64::API_SCHEMA);
}
