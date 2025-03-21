//! Module for rendering notification templates.

use std::{fmt::Display, time::Duration};

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext,
    RenderError as HandlebarsRenderError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;

use proxmox_human_byte::HumanByte;
use proxmox_time::TimeSpan;

use crate::{context, Error};

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

fn handlebars_relative_percentage_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param0 = h
        .param(0)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| HandlebarsRenderError::new("relative-percentage: param0 not found"))?;
    let param1 = h
        .param(1)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| HandlebarsRenderError::new("relative-percentage: param1 not found"))?;

    if param1 == 0.0 {
        out.write("-")?;
    } else {
        out.write(&format!("{:.2}%", (param0 * 100.0) / param1))?;
    }
    Ok(())
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
            error!("could not render value {value} with renderer {self:?}");
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

/// Choose between the provided `vendor` template or its by the user optionally created `override`
#[derive(Copy, Clone)]
pub enum TemplateSource {
    Vendor,
    Override,
}

impl Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateSource::Vendor => f.write_str("vendor"),
            TemplateSource::Override => f.write_str("override"),
        }
    }
}

/// Available template types
#[derive(Copy, Clone, PartialEq)]
pub enum TemplateType {
    /// HTML body template
    HtmlBody,
    /// Fallback HTML body, based on the `PlaintextBody` template
    HtmlBodyFromPlaintext,
    /// Plaintext body template
    PlaintextBody,
    /// Subject template
    Subject,
}

impl TemplateType {
    fn file_suffix(&self) -> &'static str {
        match self {
            TemplateType::HtmlBody => "body.html.hbs",
            TemplateType::HtmlBodyFromPlaintext => "body.txt.hbs",
            TemplateType::PlaintextBody => "body.txt.hbs",
            TemplateType::Subject => "subject.txt.hbs",
        }
    }

    fn postprocess(&self, mut rendered: String) -> String {
        match self {
            TemplateType::HtmlBodyFromPlaintext => {
                rendered = format!(
                    "<html><body><pre>{}</pre></body></html>",
                    handlebars::html_escape(&rendered)
                )
            }
            TemplateType::Subject => {
                rendered = rendered.replace('\n', " ");
            }
            _ => {}
        }

        rendered
    }

    fn block_render_fns(&self) -> BlockRenderFunctions {
        match self {
            TemplateType::HtmlBody => html::block_render_functions(),
            TemplateType::HtmlBodyFromPlaintext => plaintext::block_render_functions(),
            TemplateType::Subject => plaintext::block_render_functions(),
            TemplateType::PlaintextBody => plaintext::block_render_functions(),
        }
    }

    fn escape_fn(&self) -> fn(&str) -> String {
        match self {
            TemplateType::PlaintextBody => handlebars::no_escape,
            TemplateType::Subject => handlebars::no_escape,
            TemplateType::HtmlBody => handlebars::html_escape,
            TemplateType::HtmlBodyFromPlaintext => handlebars::no_escape,
        }
    }
}

type HelperFn = dyn HelperDef + Send + Sync;

struct BlockRenderFunctions {
    table: Box<HelperFn>,
    object: Box<HelperFn>,
}

impl BlockRenderFunctions {
    fn register_helpers(self, handlebars: &mut Handlebars) {
        handlebars.register_helper("table", self.table);
        handlebars.register_helper("object", self.object);
    }
}

fn render_template_impl(
    data: &Value,
    renderer: TemplateType,
    filename: &str,
    source: TemplateSource,
) -> Result<Option<String>, Error> {
    let template_string = context::context().lookup_template(&filename, None, source)?;

    if let Some(template_string) = template_string {
        let mut handlebars = Handlebars::new();
        handlebars.register_escape_fn(renderer.escape_fn());

        let block_render_fns = renderer.block_render_fns();
        block_render_fns.register_helpers(&mut handlebars);

        ValueRenderFunction::register_helpers(&mut handlebars);

        handlebars.register_helper(
            "relative-percentage",
            Box::new(handlebars_relative_percentage_helper),
        );

        let rendered_template = handlebars
            .render_template(&template_string, data)
            .map_err(|err| Error::RenderError(err.into()))?;

        let rendered_template = renderer.postprocess(rendered_template);

        Ok(Some(rendered_template))
    } else {
        Ok(None)
    }
}

/// Render a template string.
///
/// The output format is chosen via the `ty` parameter (see [TemplateType] for
/// available options). If an override template is found and renderable, it is
/// used instead of the vendor one. If the [TemplateType] is `HtmlBody` but no
/// HTML template is found or renderable, it falls back to use a plaintext
/// template encapsulated in a pre-formatted HTML block (<pre>).
pub fn render_template(
    mut ty: TemplateType,
    template: &str,
    data: &Value,
) -> Result<String, Error> {
    let mut source = TemplateSource::Override;

    loop {
        let filename = format!("{template}-{suffix}", suffix = ty.file_suffix());
        let result = render_template_impl(data, ty, &filename, source);

        match result {
            Ok(Some(s)) => {
                return Ok(s);
            }
            Ok(None) => {}
            Err(err) => {
                tracing::error!("failed to render {source} template '{filename}': {err}");
            }
        }

        match (ty, source) {
            (
                TemplateType::HtmlBody
                | TemplateType::HtmlBodyFromPlaintext
                | TemplateType::PlaintextBody
                | TemplateType::Subject,
                TemplateSource::Override,
            ) => {
                // Override template not found or renderable, try the vendor one instead
                source = TemplateSource::Vendor;
            }
            (TemplateType::HtmlBody, TemplateSource::Vendor) => {
                // Override and vendor HTML templates not found or renderable,
                // try next the override plaintext as fallback
                ty = TemplateType::HtmlBodyFromPlaintext;
                source = TemplateSource::Override;
            }
            (
                TemplateType::HtmlBodyFromPlaintext
                | TemplateType::PlaintextBody
                | TemplateType::Subject,
                TemplateSource::Vendor,
            ) => {
                // Return error, no suitable templates found or renderable
                break;
            }
        }
    }

    Err(Error::Generic(
        "failed to render notification template, all template candidates are erroneous or missing"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_helpers() {
        assert_eq!(value_to_byte_size(&json!(1024)), Some("1 KiB".to_string()));
        assert_eq!(
            value_to_byte_size(&json!("1024")),
            Some("1 KiB".to_string())
        );

        assert_eq!(value_to_duration(&json!(60)), Some("1m".to_string()));
        assert_eq!(value_to_duration(&json!("60")), Some("1m".to_string()));

        // The rendered value is in localtime, so we only check if the result is `Some`...
        // ... otherwise the test will break in another timezone :S
        assert!(value_to_timestamp(&json!(60)).is_some());
        assert!(value_to_timestamp(&json!("60")).is_some());
    }
}
