//! This should test the usage of "external" schemas. If a property is declared with a path instead
//! of an object, we expect the path to lead to a schema.

use proxmox::api::schema;
use proxmox_api_macro::api;

use failure::Error;

pub const NAME_SCHEMA: schema::Schema = schema::StringSchema::new("Archive name.")
    //.format(&FILENAME_FORMAT)
    .schema();

#[api(
    input: {
        properties: {
            name: {
                schema: NAME_SCHEMA,
            }
        }
    }
)]
/// Get an archive.
pub fn get_archive(name: String) -> Result<(), Error> {
    let _ = name;
    Ok(())
}
