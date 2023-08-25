use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext,
    RenderError as HandlebarsRenderError,
};
use serde_json::Value;

use super::{table::Table, value_to_string};
use crate::define_helper_with_prefix_and_postfix;
use crate::renderer::BlockRenderFunctions;

fn render_html_table(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| HandlebarsRenderError::new("parameter not found"))?;

    let value = param.value();

    let table: Table = serde_json::from_value(value.clone())?;

    out.write("<table style=\"border: 1px solid\";border-style=\"collapse\">\n")?;

    // Write header
    out.write("  <tr>\n")?;
    for column in &table.schema.columns {
        out.write("    <th style=\"border: 1px solid\">")?;
        out.write(&handlebars::html_escape(&column.label))?;
        out.write("</th>\n")?;
    }
    out.write("  </tr>\n")?;

    // Write individual rows
    for row in &table.data {
        out.write("  <tr>\n")?;

        for column in &table.schema.columns {
            let entry = row.get(&column.id).unwrap_or(&Value::Null);

            let text = if let Some(renderer) = &column.renderer {
                renderer.render(entry)
            } else {
                value_to_string(entry)
            };

            out.write("    <td style=\"border: 1px solid\">")?;
            out.write(&handlebars::html_escape(&text))?;
            out.write("</td>\n")?;
        }
        out.write("  </tr>\n")?;
    }

    out.write("</table>\n")?;

    Ok(())
}

fn render_object(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| HandlebarsRenderError::new("parameter not found"))?;

    let value = param.value();

    out.write("\n<pre>")?;
    out.write(&serde_json::to_string_pretty(&value)?)?;
    out.write("\n</pre>\n")?;

    Ok(())
}

define_helper_with_prefix_and_postfix!(verbatim_monospaced, "<pre>", "</pre>");
define_helper_with_prefix_and_postfix!(heading_1, "<h1 style=\"font-size: 1.2em\">", "</h1>");
define_helper_with_prefix_and_postfix!(heading_2, "<h2 style=\"font-size: 1em\">", "</h2>");
define_helper_with_prefix_and_postfix!(
    verbatim,
    "<pre style=\"font-family: sans-serif\">",
    "</pre>"
);

pub(super) fn block_render_functions() -> BlockRenderFunctions {
    BlockRenderFunctions {
        table: Box::new(render_html_table),
        verbatim_monospaced: Box::new(verbatim_monospaced),
        object: Box::new(render_object),
        heading_1: Box::new(heading_1),
        heading_2: Box::new(heading_2),
        verbatim: Box::new(verbatim),
    }
}
