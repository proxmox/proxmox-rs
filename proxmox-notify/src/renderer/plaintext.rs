use crate::define_helper_with_prefix_and_postfix;
use crate::renderer::BlockRenderFunctions;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext,
    RenderError as HandlebarsRenderError,
};
use serde_json::Value;
use std::collections::HashMap;

use super::{table::Table, value_to_string};

fn optimal_column_widths(table: &Table) -> HashMap<&str, usize> {
    let mut widths = HashMap::new();

    for column in &table.schema.columns {
        let mut min_width = column.label.len();

        for row in &table.data {
            let entry = row.get(&column.id).unwrap_or(&Value::Null);

            let text = if let Some(renderer) = &column.renderer {
                renderer.render(entry).unwrap_or_default()
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
        .ok_or_else(|| HandlebarsRenderError::new("parameter not found"))?;
    let value = param.value();
    let table: Table = serde_json::from_value(value.clone())?;
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
                renderer.render(entry)?
            } else {
                value_to_string(entry)
            };

            out.write(&format!("{text:width$}",))?;
        }
        out.write("\n")?;
    }

    Ok(())
}

macro_rules! define_underlining_heading_fn {
    ($name:ident, $underline:expr) => {
        fn $name<'reg, 'rc>(
            h: &Helper<'reg, 'rc>,
            _handlebars: &'reg Handlebars,
            _context: &'rc Context,
            _render_context: &mut RenderContext<'reg, 'rc>,
            out: &mut dyn Output,
        ) -> HelperResult {
            let param = h
                .param(0)
                .ok_or_else(|| HandlebarsRenderError::new("No parameter provided"))?;

            let value = param.value();
            let text = value.as_str().ok_or_else(|| {
                HandlebarsRenderError::new(format!("value {value} is not a string"))
            })?;

            out.write(text)?;
            out.write("\n")?;

            for _ in 0..text.len() {
                out.write($underline)?;
            }
            Ok(())
        }
    };
}

define_helper_with_prefix_and_postfix!(verbatim_monospaced, "", "");
define_underlining_heading_fn!(heading_1, "=");
define_underlining_heading_fn!(heading_2, "-");
define_helper_with_prefix_and_postfix!(verbatim, "", "");

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

    out.write("\n")?;
    out.write(&serde_json::to_string_pretty(&value)?)?;
    out.write("\n")?;

    Ok(())
}

pub(super) fn block_render_functions() -> BlockRenderFunctions {
    BlockRenderFunctions {
        table: Box::new(render_plaintext_table),
        verbatim_monospaced: Box::new(verbatim_monospaced),
        verbatim: Box::new(verbatim),
        object: Box::new(render_object),
        heading_1: Box::new(heading_1),
        heading_2: Box::new(heading_2),
    }
}
