use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};
use serde_json::Value;

use super::{table::Table, value_to_string};
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
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("table", 0))?;

    let value = param.value();

    let table: Table = serde_json::from_value(value.clone())
        .map_err(|err| RenderErrorReason::NestedError(err.into()))?;

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
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("object", 0))?;

    let value = param.value();

    out.write("\n<pre>")?;
    out.write(
        &serde_json::to_string_pretty(&value)
            .map_err(|err| RenderErrorReason::NestedError(err.into()))?,
    )?;
    out.write("\n</pre>\n")?;

    Ok(())
}

pub(super) fn block_render_functions() -> BlockRenderFunctions {
    BlockRenderFunctions {
        table: Box::new(render_html_table),
        object: Box::new(render_object),
    }
}
