//! Module to generate and format API Documenation

use std::io::Write;

use anyhow::Error;

use proxmox_schema::format::*;
use proxmox_schema::ObjectSchemaType;

use crate::{ApiHandler, ApiMethod};

fn dump_method_definition(method: &str, path: &str, def: Option<&ApiMethod>) -> Option<String> {
    let style = ParameterDisplayStyle::Config;
    match def {
        None => None,
        Some(api_method) => {
            let description = wrap_text("", "", &api_method.parameters.description(), 80);
            let param_descr = dump_properties(&api_method.parameters, "", style, &[]);

            let return_descr = dump_api_return_schema(&api_method.returns, style);

            let mut method = method;

            if let ApiHandler::AsyncHttp(_) = api_method.handler {
                method = if method == "POST" { "UPLOAD" } else { method };
                method = if method == "GET" { "DOWNLOAD" } else { method };
            }

            let res = format!(
                "**{} {}**\n\n{}{}\n\n{}",
                method, path, description, param_descr, return_descr
            );
            Some(res)
        }
    }
}

/// Generate ReST Documentaion for a complete API defined by a ``Router``.
pub fn dump_api(
    output: &mut dyn Write,
    router: &crate::Router,
    path: &str,
    mut pos: usize,
) -> Result<(), Error> {
    use crate::SubRoute;

    let mut cond_print = |x| -> Result<_, Error> {
        if let Some(text) = x {
            if pos > 0 {
                writeln!(output, "-----\n")?;
            }
            writeln!(output, "{}", text)?;
            pos += 1;
        }
        Ok(())
    };

    cond_print(dump_method_definition("GET", path, router.get))?;
    cond_print(dump_method_definition("POST", path, router.post))?;
    cond_print(dump_method_definition("PUT", path, router.put))?;
    cond_print(dump_method_definition("DELETE", path, router.delete))?;

    match &router.subroute {
        None => return Ok(()),
        Some(SubRoute::MatchAll { router, param_name }) => {
            let sub_path = if path == "." {
                format!("<{}>", param_name)
            } else {
                format!("{}/<{}>", path, param_name)
            };
            dump_api(output, router, &sub_path, pos)?;
        }
        Some(SubRoute::Map(dirmap)) => {
            //let mut keys: Vec<&String> = map.keys().collect();
            //keys.sort_unstable_by(|a, b| a.cmp(b));
            for (key, sub_router) in dirmap.iter() {
                let sub_path = if path == "." {
                    (*key).to_string()
                } else {
                    format!("{}/{}", path, key)
                };
                dump_api(output, sub_router, &sub_path, pos)?;
            }
        }
    }

    Ok(())
}
