use proxmox_notify::renderer::{render_template, TemplateRenderer};
use proxmox_notify::Error;

use serde_json::json;

const TEMPLATE: &str = r#"
{{ heading-1 "Backup Report"}}
A backup job on host {{host}} was run.

{{ heading-2 "Guests"}}
{{ table table }}
The total size of all backups is {{human-bytes total-size}}.

The backup job took {{duration total-time}}.

{{ heading-2 "Logs"}}
{{ verbatim-monospaced logs}}

{{ heading-2 "Objects"}}
{{ object table }}
"#;

fn main() -> Result<(), Error> {
    let properties = json!({
        "host": "pali",
        "logs": "100: starting backup\n100: backup failed",
        "total-size": 1024 * 1024 + 2048 * 1024,
        "total-time": 100,
        "table": {
            "schema": {
                "columns": [
                    {
                        "label": "VMID",
                        "id": "vmid"
                    },
                    {
                        "label": "Size",
                        "id": "size",
                        "renderer": "human-bytes"
                    }
                ],
            },
            "data" : [
                {
                    "vmid": 1001,
                    "size": "1048576"
                },
                {
                    "vmid": 1002,
                    "size": 2048 * 1024,
                }
            ]
        }
    });

    let output = render_template(TemplateRenderer::Html, TEMPLATE, &properties)?;
    println!("{output}");

    let output = render_template(TemplateRenderer::Plaintext, TEMPLATE, &properties)?;
    println!("{output}");

    Ok(())
}
