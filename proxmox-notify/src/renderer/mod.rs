//! Module for rendering notification templates.

use std::time::Duration;

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext,
    RenderError as HandlebarsRenderError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use proxmox_human_byte::HumanByte;
use proxmox_time::TimeSpan;

use crate::Error;

mod html;
mod plaintext;
mod table;

/// Convert a serde_json::Value to a String.
///
/// The main difference between this and simply calling Value::to_string is that
/// this will print strings without double quotes
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}

/// Render a `serde_json::Value` as a byte size with proper units (IEC, base 2).
/// Accepts `serde_json::Value::{Number,String}`.
///
/// Will return `None` if `val` does not contain a number/parseable string.
fn value_to_byte_size(val: &Value) -> Option<String> {
    let size = match val {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }?;

    Some(format!("{}", HumanByte::new_binary(size)))
}

/// Render a serde_json::Value as a duration.
/// The value is expected to contain the duration in seconds.
/// Accepts `serde_json::Value::{Number,String}`.
///
/// Will return `None` if `val` does not contain a number/parseable string.
fn value_to_duration(val: &Value) -> Option<String> {
    let duration = match val {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }?;
    let time_span = TimeSpan::from(Duration::from_secs(duration));

    Some(format!("{time_span}"))
}

/// Render as serde_json::Value as a timestamp.
/// The value is expected to contain the timestamp as a unix epoch.
/// Accepts `serde_json::Value::{Number,String}`.
///
/// Will return `None` if `val` does not contain a number/parseable string.
fn value_to_timestamp(val: &Value) -> Option<String> {
    let timestamp = match val {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }?;
    proxmox_time::strftime_local("%F %H:%M:%S", timestamp).ok()
}

/// Available render functions for `serde_json::Values``
///
/// May be used as a handlebars helper, e.g.
/// ```text
/// {{human-bytes 1024}}
/// ```
///
/// Value renderer can also be used for rendering values in table columns:
/// ```text
/// let properties = json!({
///     "table": {
///         "schema": {
///             "columns": [
///                 {
///                     "label": "Size",
///                     "id": "size",
///                     "renderer": "human-bytes"
///                 }
///             ],
///         },
///         "data" : [
///             {
///                 "size": 1024 * 1024,
///             },
///         ]
///     }
/// });
/// ```
///
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValueRenderFunction {
    HumanBytes,
    Duration,
    Timestamp,
}

impl ValueRenderFunction {
    fn render(&self, value: &Value) -> String {
        match self {
            ValueRenderFunction::HumanBytes => value_to_byte_size(value),
            ValueRenderFunction::Duration => value_to_duration(value),
            ValueRenderFunction::Timestamp => value_to_timestamp(value),
        }
        .unwrap_or_else(|| {
            log::error!("could not render value {value} with renderer {self:?}");
            String::from("ERROR")
        })
    }

    fn register_helpers(handlebars: &mut Handlebars) {
        ValueRenderFunction::HumanBytes.register_handlebars_helper(handlebars);
        ValueRenderFunction::Duration.register_handlebars_helper(handlebars);
        ValueRenderFunction::Timestamp.register_handlebars_helper(handlebars);
    }

    fn register_handlebars_helper(&'static self, handlebars: &mut Handlebars) {
        // Use serde to get own kebab-case representation that is later used
        // to register the helper, e.g. HumanBytes -> human-bytes
        let tag = serde_json::to_string(self)
            .expect("serde failed to serialize ValueRenderFunction enum");

        // But as it's a string value, the generated string is quoted,
        // so remove leading/trailing double quotes
        let tag = tag
            .strip_prefix('\"')
            .and_then(|t| t.strip_suffix('\"'))
            .expect("serde serialized string representation was not contained in double quotes");

        handlebars.register_helper(
            tag,
            Box::new(
                |h: &Helper,
                 _r: &Handlebars,
                 _: &Context,
                 _rc: &mut RenderContext,
                 out: &mut dyn Output|
                 -> HelperResult {
                    let param = h
                        .param(0)
                        .ok_or(HandlebarsRenderError::new("parameter not found"))?;

                    let value = param.value();
                    out.write(&self.render(value))?;

                    Ok(())
                },
            ),
        );
    }
}

/// Available renderers for notification templates.
#[derive(Copy, Clone)]
pub enum TemplateRenderer {
    /// Render to HTML code
    Html,
    /// Render to plain text
    Plaintext,
}

impl TemplateRenderer {
    fn prefix(&self) -> &str {
        match self {
            TemplateRenderer::Html => "<html>\n<body>\n",
            TemplateRenderer::Plaintext => "",
        }
    }

    fn postfix(&self) -> &str {
        match self {
            TemplateRenderer::Html => "\n</body>\n</html>",
            TemplateRenderer::Plaintext => "",
        }
    }

    fn block_render_fns(&self) -> BlockRenderFunctions {
        match self {
            TemplateRenderer::Html => html::block_render_functions(),
            TemplateRenderer::Plaintext => plaintext::block_render_functions(),
        }
    }

    fn escape_fn(&self) -> fn(&str) -> String {
        match self {
            TemplateRenderer::Html => handlebars::html_escape,
            TemplateRenderer::Plaintext => handlebars::no_escape,
        }
    }
}

type HelperFn = dyn HelperDef + Send + Sync;

struct BlockRenderFunctions {
    table: Box<HelperFn>,
    verbatim_monospaced: Box<HelperFn>,
    object: Box<HelperFn>,
    heading_1: Box<HelperFn>,
    heading_2: Box<HelperFn>,
    verbatim: Box<HelperFn>,
}

impl BlockRenderFunctions {
    fn register_helpers(self, handlebars: &mut Handlebars) {
        handlebars.register_helper("table", self.table);
        handlebars.register_helper("verbatim", self.verbatim);
        handlebars.register_helper("verbatim-monospaced", self.verbatim_monospaced);
        handlebars.register_helper("object", self.object);
        handlebars.register_helper("heading-1", self.heading_1);
        handlebars.register_helper("heading-2", self.heading_2);
    }
}

fn render_template_impl(
    template: &str,
    properties: Option<&Value>,
    renderer: TemplateRenderer,
) -> Result<String, Error> {
    let properties = properties.unwrap_or(&Value::Null);

    let mut handlebars = Handlebars::new();
    handlebars.register_escape_fn(renderer.escape_fn());

    let block_render_fns = renderer.block_render_fns();
    block_render_fns.register_helpers(&mut handlebars);

    ValueRenderFunction::register_helpers(&mut handlebars);

    let rendered_template = handlebars
        .render_template(template, properties)
        .map_err(|err| Error::RenderError(err.into()))?;

    Ok(rendered_template)
}

/// Render a template string.
///
/// The output format can be chosen via the `renderer` parameter (see [TemplateRenderer]
/// for available options).
pub fn render_template(
    renderer: TemplateRenderer,
    template: &str,
    properties: Option<&Value>,
) -> Result<String, Error> {
    let mut rendered_template = String::from(renderer.prefix());

    rendered_template.push_str(&render_template_impl(template, properties, renderer)?);
    rendered_template.push_str(renderer.postfix());

    Ok(rendered_template)
}

#[macro_export]
macro_rules! define_helper_with_prefix_and_postfix {
    ($name:ident, $pre:expr, $post:expr) => {
        fn $name<'reg, 'rc>(
            h: &Helper<'reg, 'rc>,
            handlebars: &'reg Handlebars,
            context: &'rc Context,
            render_context: &mut RenderContext<'reg, 'rc>,
            out: &mut dyn Output,
        ) -> HelperResult {
            use handlebars::Renderable;

            let block_text = h.template();
            let param = h.param(0);

            out.write($pre)?;
            match (param, block_text) {
                (None, Some(block_text)) => {
                    block_text.render(handlebars, context, render_context, out)
                }
                (Some(param), None) => {
                    let value = param.value();
                    let text = value.as_str().ok_or_else(|| {
                        HandlebarsRenderError::new(format!("value {value} is not a string"))
                    })?;

                    out.write(text)?;
                    Ok(())
                }
                (Some(_), Some(_)) => Err(HandlebarsRenderError::new(
                    "Cannot use parameter and template at the same time",
                )),
                (None, None) => Err(HandlebarsRenderError::new(
                    "Neither parameter nor template was provided",
                )),
            }?;
            out.write($post)?;
            Ok(())
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_template() -> Result<(), Error> {
        let properties = json!({
            "dur": 12345,
            "size": 1024 * 15,

            "table": {
                "schema": {
                    "columns": [
                        {
                            "id": "col1",
                            "label": "Column 1"
                        },
                        {
                            "id": "col2",
                            "label": "Column 2"
                        }
                    ]
                },
                "data": [
                    {
                        "col1": "val1",
                        "col2": "val2"
                    },
                    {
                        "col1": "val3",
                        "col2": "val4"
                    },
                ]
            }

        });

        let template = r#"
{{heading-1 "Hello World"}}

{{heading-2 "Hello World"}}

{{human-bytes size}}
{{duration dur}}

{{table table}}"#;

        let expected_plaintext = r#"
Hello World
===========

Hello World
-----------

15 KiB
3h 25min 45s

Column 1    Column 2    
val1        val2        
val3        val4        
"#;

        let rendered_plaintext =
            render_template(TemplateRenderer::Plaintext, template, Some(&properties))?;

        // Let's not bother about testing the HTML output, too fragile.

        assert_eq!(rendered_plaintext, expected_plaintext);

        Ok(())
    }

    #[test]
    fn test_helpers() {
        assert_eq!(value_to_byte_size(&json!(1024)), Some("1 KiB".to_string()));
        assert_eq!(
            value_to_byte_size(&json!("1024")),
            Some("1 KiB".to_string())
        );

        assert_eq!(value_to_duration(&json!(60)), Some("1min ".to_string()));
        assert_eq!(value_to_duration(&json!("60")), Some("1min ".to_string()));

        // The rendered value is in localtime, so we only check if the result is `Some`...
        // ... otherwise the test will break in another timezone :S
        assert!(value_to_timestamp(&json!(60)).is_some());
        assert!(value_to_timestamp(&json!("60")).is_some());
    }
}
