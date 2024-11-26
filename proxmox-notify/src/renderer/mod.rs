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

/// Available template types
#[derive(Copy, Clone)]
pub enum TemplateType {
    /// HTML body template
    HtmlBody,
    /// Plaintext body template
    PlaintextBody,
    /// Subject template
    Subject,
}

impl TemplateType {
    fn file_suffix(&self) -> &'static str {
        match self {
            TemplateType::HtmlBody => "body.html.hbs",
            TemplateType::PlaintextBody => "body.txt.hbs",
            TemplateType::Subject => "subject.txt.hbs",
        }
    }

    fn postprocess(&self, mut rendered: String) -> String {
        if let Self::Subject = self {
            rendered = rendered.replace('\n', " ");
        }

        rendered
    }

    fn block_render_fns(&self) -> BlockRenderFunctions {
        match self {
            TemplateType::HtmlBody => html::block_render_functions(),
            TemplateType::Subject => plaintext::block_render_functions(),
            TemplateType::PlaintextBody => plaintext::block_render_functions(),
        }
    }

    fn escape_fn(&self) -> fn(&str) -> String {
        match self {
            TemplateType::PlaintextBody => handlebars::no_escape,
            TemplateType::Subject => handlebars::no_escape,
            TemplateType::HtmlBody => handlebars::html_escape,
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
    template: &str,
    data: &Value,
    renderer: TemplateType,
) -> Result<String, Error> {
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
        .render_template(template, data)
        .map_err(|err| Error::RenderError(err.into()))?;

    Ok(rendered_template)
}

/// Render a template string.
///
/// The output format can be chosen via the `renderer` parameter (see [TemplateType]
/// for available options).
pub fn render_template(
    mut ty: TemplateType,
    template: &str,
    data: &Value,
) -> Result<String, Error> {
    let filename = format!("{template}-{suffix}", suffix = ty.file_suffix());

    let template_string = context::context().lookup_template(&filename, None)?;

    let (template_string, fallback) = match (template_string, ty) {
        (None, TemplateType::HtmlBody) => {
            ty = TemplateType::PlaintextBody;
            let plaintext_filename = format!("{template}-{suffix}", suffix = ty.file_suffix());
            (
                context::context().lookup_template(&plaintext_filename, None)?,
                true,
            )
        }
        (template_string, _) => (template_string, false),
    };

    let template_string = template_string.ok_or(Error::Generic(format!(
        "could not load template '{template}'"
    )))?;

    let mut rendered = render_template_impl(&template_string, data, ty)?;
    rendered = ty.postprocess(rendered);

    if fallback {
        rendered = format!(
            "<html><body><pre>{}</pre></body></html>",
            handlebars::html_escape(&rendered)
        );
    }

    Ok(rendered)
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
