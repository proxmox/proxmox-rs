use std::collections::HashMap;

use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};
use serde_json::Value;

use super::{table::Table, value_to_string};
use crate::renderer::BlockRenderFunctions;

fn optimal_column_widths(table: &Table) -> HashMap<&str, usize> {
    let mut widths = HashMap::new();

    for column in &table.schema.columns {
        let mut min_width = column.label.len();

        for row in &table.data {
            let entry = row.get(&column.id).unwrap_or(&Value::Null);

            let text = if let Some(renderer) = &column.renderer {
                renderer.render(entry)
            } else {
                value_to_string(entry)
            };

            min_width = std::cmp::max(text.len(), min_width);
        }

        widths.insert(column.label.as_str(), min_width + 4);
    }

    widths
}

fn render_plaintext_table(
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
    let widths = optimal_column_widths(&table);

    // Write header
    for column in &table.schema.columns {
        let width = widths.get(column.label.as_str()).unwrap_or(&0);
        out.write(&format!("{label:width$}", label = column.label))?;
    }

    out.write("\n")?;

    // Write individual rows
    for row in &table.data {
        for column in &table.schema.columns {
            let entry = row.get(&column.id).unwrap_or(&Value::Null);
            let width = widths.get(column.label.as_str()).unwrap_or(&0);

            let text = if let Some(renderer) = &column.renderer {
                renderer.render(entry)
            } else {
                value_to_string(entry)
            };

            out.write(&format!("{text:width$}",))?;
        }
        out.write("\n")?;
    }

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

    out.write("\n")?;
    out.write(
        &serde_json::to_string_pretty(&value)
            .map_err(|err| RenderErrorReason::NestedError(err.into()))?,
    )?;
    out.write("\n")?;

    Ok(())
}

pub(super) fn block_render_functions() -> BlockRenderFunctions {
    BlockRenderFunctions {
        table: Box::new(render_plaintext_table),
        object: Box::new(render_object),
    }
}
