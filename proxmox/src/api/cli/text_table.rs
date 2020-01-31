use std::io::Write;

use failure::*;
use serde_json::Value;

use crate::api::schema::*;

/// Render function
///
/// Should convert the json `value` into a text string. `record` points to
/// the surrounding data object.
pub type RenderFunction = fn(/* value: */ &Value, /* record: */ &Value) -> Result<String, Error>;

fn data_to_text(data: &Value, schema: &Schema) -> Result<String, Error> {

    if data.is_null() { return Ok(String::new()); }

    match schema {
        Schema::Null => {
            // makes no sense to display Null columns
            bail!("internal error");
        }
        Schema::Boolean(_boolean_schema) => {
            match data.as_bool() {
                Some(value) => {
                    Ok(String::from(if value { "1" } else { "0" }))
                }
                None => bail!("got unexpected data (expected bool)."),
            }
        }
        Schema::Integer(_integer_schema) => {
            match data.as_i64() {
                Some(value) => {
                    Ok(format!("{}", value))
                }
                None => bail!("got unexpected data (expected integer)."),
            }
        }
        Schema::Number(_number_schema) => {
            match data.as_f64() {
                Some(value) => {
                    Ok(format!("{}", value))
                }
                None => bail!("got unexpected data (expected number)."),
            }
        }
        Schema::String(_string_schema) => {
            match data.as_str() {
                Some(value) => {
                    Ok(format!("{}", value))
                }
                None => bail!("got unexpected data (expected string)."),
            }
        }
        Schema::Object(_object_schema) => {
            Ok(data.to_string())
        }
        Schema::Array(_array_schema) => {
            Ok(data.to_string())
        }
    }
}

struct TableBorders {
    column_separator: char,
    top: String,
    head: String,
    middle: String,
    bottom: String,
}

impl TableBorders {

    fn new(column_widths: &Vec<usize>, ascii_delimiters: bool) -> Self {

        let mut top = String::new();
        let mut head = String::new();
        let mut middle = String::new();
        let mut bottom = String::new();

        let column_separator = '│';

        for (i, column_width) in column_widths.iter().enumerate() {
            if ascii_delimiters {
                top.push('+');
                head.push('+');
                middle.push('+');
                bottom.push('+');
            } else {
                if i == 0 {
                    top.push('┌');
                    head.push('╞');
                    middle.push('├');
                    bottom.push('└');
                } else {
                    top.push('┬');
                    head.push('╪');
                    middle.push('┼');
                    bottom.push('┴');
                }
            }

            for _j in 0..*column_width {
                if ascii_delimiters {
                    top.push('=');
                    head.push('=');
                    middle.push('-');
                    bottom.push('=');
                } else {
                    top.push('─');
                    head.push('═');
                    middle.push('─');
                    bottom.push('─');
                }
            }
        }
        if ascii_delimiters {
            top.push('+');
            head.push('+');
            middle.push('+');
            bottom.push('+');
        } else {
            top.push('┐');
            head.push('╡');
            middle.push('┤');
            bottom.push('┘');
        }

        Self {
            column_separator,
            top,
            head,
            middle,
            bottom,
        }
    }
}

/// Table Column configuration
///
/// This structure can be used to set additional rendering information for a table column.
pub struct ColumnConfig {
    pub name: String,
    pub right_align: Option<bool>,
    pub renderer: Option<RenderFunction>,
}

impl ColumnConfig {

    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            right_align: None,
            renderer: None,
        }
    }

    pub fn right_align(mut self, right_align: bool) -> Self {
        self.right_align = Some(right_align);
        self
    }

    pub fn renderer(mut self, renderer: RenderFunction) -> Self {
        self.renderer = Some(renderer);
        self
    }
}

/// Table formatter configuration
#[derive(Default)]
pub struct TableFormatOptions {
    /// Can be used to sort after a specific column, if it isn't set we sort
    /// after the leftmost column (with no undef value in $data) this can be
    /// turned off by passing "" as sort_key.
    pub sortkey: Option<String>,
    /// Print without asciiart border.
    pub noborder: bool,
    /// Print without table header.
    pub noheader: bool,
    /// Limit output width.
    pub columns: Option<usize>,
    /// Use ascii characters for table delimiters (instead of utf8).
    pub ascii_delimiters: bool,
    /// Comumn configurations
    pub column_config: Vec<ColumnConfig>,
}

impl TableFormatOptions {

    /// Create a new Instance with reasonable defaults for terminal output
    ///
    /// This tests if stdout is a TTY and sets the columns toö the terminal width,
    /// and sets ascii_delimiters toö true If the locale CODESET is not UTF-8.
    pub fn new() -> Self {
        let mut me = Self::default();

        let is_tty = unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 };

        if is_tty {
            let (_rows, columns) = crate::sys::linux::tty::stdout_terminal_size();
            if columns > 0 {
                me.columns = Some(columns);
            }
        }

        let empty_cstr = crate::c_str!("");

        use std::ffi::CStr;
        let encoding = unsafe {
            libc::setlocale(libc::LC_CTYPE, empty_cstr.as_ptr());
            CStr::from_ptr(libc::nl_langinfo(libc::CODESET))
        };

        if encoding != crate::c_str!("UTF-8") {
            me.ascii_delimiters = true;
        }

        me
    }

    pub fn sortkey(mut self, sortkey: Option<String>) -> Self {
        self.sortkey = sortkey;
        self
    }
    pub fn noborder(mut self, noborder: bool) -> Self {
        self.noborder = noborder;
        self
    }

    pub fn noheader(mut self, noheader: bool) -> Self {
        self.noheader = noheader;
        self
    }

    pub fn ascii_delimiters(mut self, ascii_delimiters: bool) -> Self {
        self.ascii_delimiters = ascii_delimiters;
        self
    }

    pub fn columns(mut self, columns: Option<usize>) -> Self {
        self.columns = columns;
        self
    }

    pub fn column_config(mut self, column_config: Vec<ColumnConfig>) -> Self {
        self.column_config = column_config;
        self
    }

    /// Add a single column configuration
    pub fn column(mut self, column_config: ColumnConfig) -> Self {
        self.column_config.push(column_config);
        self
    }
}

struct TableCell {
    lines: Vec<String>,
}

struct TableColumn {
    cells: Vec<TableCell>,
    width: usize,
    right_align: bool,
}

fn format_table<W: Write>(
    output: W,
    list: &mut Vec<Value>,
    schema: &ObjectSchema,
    options: &TableFormatOptions,
) -> Result<(), Error> {

    let properties_to_print = if options.column_config.is_empty() {
        extract_properties_to_print(schema)
    } else {
        options.column_config.iter().map(|v| v.name.clone()).collect()
    };

    let column_count = properties_to_print.len();
    if column_count == 0 { return Ok(()); };

    let sortkey = if let Some(ref sortkey) = options.sortkey {
        sortkey.clone()
    } else {
        properties_to_print[0].clone() // leftmost
    };

    let (_optional, sort_prop_schema) = match schema.lookup(&sortkey) {
        Some(tup) => tup,
        None => bail!("property {} does not exist in schema.", sortkey),
    };

    let numeric_sort = match sort_prop_schema {
        Schema::Integer(_) => true,
        Schema::Number(_) => true,
        _ => false,
    };

    if numeric_sort {
        use std::cmp::Ordering;
        list.sort_unstable_by(move |a, b| {
            let d1 = a[&sortkey].as_f64();
            let d2 = b[&sortkey].as_f64();
            match (d1,d2) {
                (None, None) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (Some(a), Some(b)) => {
                    if a.is_nan() { return Ordering::Greater; }
                    if b.is_nan() { return Ordering::Less; }
                    if a < b {
                        return Ordering::Less;
                    } else if a > b {
                        return Ordering::Greater;
                    }
                    return Ordering::Equal;
                }
            };
        })
    } else {
        list.sort_unstable_by_key(move |d| d[&sortkey].to_string());
    }

    let mut tabledata: Vec<TableColumn> = Vec::new();

    for name in properties_to_print.iter() {
        let (_optional, prop_schema) = match schema.lookup(name) {
            Some(tup) => tup,
            None => bail!("property {} does not exist in schema.", name),
        };

        let mut right_align = match prop_schema {
            Schema::Integer(_) => true,
            Schema::Number(_) => true,
            Schema::Boolean(_) => true,
            _ => false,
        };

        let mut renderer = None;

        if let Some(column_config) = options.column_config.iter().find(|v| v.name == *name) {
            renderer = column_config.renderer;
            right_align = column_config.right_align.unwrap_or(right_align);
        }

        let mut max_width = name.chars().count();


        let mut cells = Vec::new();
        for entry in list.iter() {
            let result = if let Some(renderer) = renderer {
                (renderer)(&entry[name], &entry)
            } else {
                data_to_text(&entry[name], prop_schema)
            };

            let text = match result {
                Ok(text) => text,
                Err(err) => bail!("unable to format property {} - {}", name, err),
            };

            let lines: Vec<String> = text.lines().map(|line| {
                let width = line.chars().count();
                if width > max_width { max_width = width; }
                line.to_string()
            }).collect();

            cells.push(TableCell { lines });
        }

        tabledata.push(TableColumn { cells, width: max_width, right_align});
    }

    render_table(output, &tabledata, &properties_to_print, options)
}

fn render_table<W: Write>(
    mut output: W,
    tabledata: &Vec<TableColumn>,
    column_names: &Vec<String>,
    options: &TableFormatOptions,
) -> Result<(), Error> {

    let mut write_line = |line: &str| -> Result<(), Error> {
        if let Some(columns) = options.columns {
            let line: String = line.chars().take(columns).collect();
            output.write_all(line.as_bytes())?;
        } else {
            output.write_all(line.as_bytes())?;
        }
        output.write_all(b"\n")?;
        Ok(())
    };

    let column_widths = tabledata.iter().map(|d| d.width).collect();

    let borders = TableBorders::new(&column_widths, options.ascii_delimiters);

    if !options.noborder { write_line(&borders.top)?; }

    let mut header = String::new();
    for (i, name) in column_names.iter().enumerate() {
        let column = &tabledata[i];
        header.push(borders.column_separator);
        if column.right_align {
            header.push_str(&format!("{:>width$}", name, width = column.width));
        } else {
            header.push_str(&format!("{:<width$}", name, width = column.width));
        }
    }

    if !(options.noheader || options.noborder) {
        header.push(borders.column_separator);

        write_line(&header)?;
        write_line(&borders.head)?;
    }

    let rows = tabledata[0].cells.len();
    for pos in 0..rows {
        let mut max_lines = 0;
        for (i, _name) in column_names.iter().enumerate() {
            let cells = &tabledata[i].cells;
            let lines = &cells[pos].lines;
            if lines.len() > max_lines { max_lines = lines.len(); }
        }
        for line_nr in 0..max_lines {
            let mut text = String::new();
            let empty_string = String::new();
            for (i, _name) in column_names.iter().enumerate() {
                let column = &tabledata[i];
                let lines = &column.cells[pos].lines;
                let line = lines.get(line_nr).unwrap_or(&empty_string);

                if options.noborder {
                    if i > 0 { text.push(' '); }
                } else {
                    text.push(borders.column_separator);
                }

                if column.right_align {
                    text.push_str(&format!("{:>width$}", line, width = column.width));
                } else {
                    text.push_str(&format!("{:<width$}", line, width = column.width));
                }
            }
            if !options.noborder { text.push(borders.column_separator); }
            write_line(&text)?;
        }

        if !options.noborder {
            if (pos + 1) == rows {
                write_line(&borders.bottom)?;
            } else {
                write_line(&borders.middle)?;
            }
        }
    }

    Ok(())
}

fn format_object<W: Write>(
    output: W,
    data: &Value,
    schema: &ObjectSchema,
    options: &TableFormatOptions,
) -> Result<(), Error> {

    let properties_to_print = if options.column_config.is_empty() {
        extract_properties_to_print(schema)
    } else {
        options.column_config.iter().map(|v| v.name.clone()).collect()
    };

    let row_count = properties_to_print.len();
    if row_count == 0 { return Ok(()); };

    const NAME_TITLE: &str = "Name";
    const VALUE_TITLE: &str = "Value";

    let mut max_name_width = NAME_TITLE.len();
    let mut max_value_width = VALUE_TITLE.len();

    let column_names = vec![NAME_TITLE.to_string(), VALUE_TITLE.to_string()];

    let mut name_cells = Vec::new();
    let mut value_cells = Vec::new();

    for name in properties_to_print.iter() {
        let (_optional, prop_schema) = match schema.lookup(name) {
            Some(tup) => tup,
            None => bail!("property {} does not exist in schema.", name),
        };

        let mut renderer = None;
        if let Some(column_config) = options.column_config.iter().find(|v| v.name == *name) {
            renderer = column_config.renderer;
        }

        let name_width = name.chars().count();
        if name_width > max_name_width { max_name_width = name_width; }

        name_cells.push(TableCell { lines: vec![ name.to_string() ] });

        let result = if let Some(renderer) = renderer {
            (renderer)(&data[name], &data)
        } else {
            data_to_text(&data[name], prop_schema)
        };

        let text = match result {
            Ok(text) => text,
            Err(err) => bail!("unable to format property {} - {}", name, err),
        };

        let lines: Vec<String> = text.lines().map(|line| {
            let width = line.chars().count();
            if width > max_value_width { max_value_width = width; }
            line.to_string()
        }).collect();

        value_cells.push(TableCell { lines });
    }

    let name_column = TableColumn { cells: name_cells, width: max_name_width, right_align: false };
    let value_column = TableColumn { cells: value_cells, width: max_value_width, right_align: false };

    let mut tabledata: Vec<TableColumn> = Vec::new();
    tabledata.push(name_column);
    tabledata.push(value_column);

    render_table(output, &tabledata, &column_names, options)
}

fn extract_properties_to_print(schema: &ObjectSchema) -> Vec<String> {
    let mut result = Vec::new();

    for (name, optional, _prop_schema) in schema.properties {
        if !*optional { result.push(name.to_string()); }
    }
    for (name, optional, _prop_schema) in schema.properties {
        if *optional { result.push(name.to_string()); }
    }
    result
}

/// Format data using TableFormatOptions
pub fn value_to_text<W: Write>(
    output: W,
    data: &mut Value,
    schema: &Schema,
    options: &TableFormatOptions,
) -> Result<(), Error> {

    match schema {
        Schema::Null => {
            if *data != Value::Null {
                bail!("got unexpected data (expected null).");
            }
        }
        Schema::Boolean(_boolean_schema) => {
            unimplemented!();
        }
        Schema::Integer(_integer_schema) => {
            unimplemented!();
        }
        Schema::Number(_number_schema) => {
            unimplemented!();
        }
        Schema::String(_string_schema) => {
            unimplemented!();
        }
        Schema::Object(object_schema) => {
            format_object(output, data, object_schema, options)?;
        }
        Schema::Array(array_schema) => {
            let list = match data.as_array_mut() {
                Some(list) => list,
                None => bail!("got unexpected data (expected array)."),
            };
            if list.is_empty() { return Ok(()); }

            match array_schema.items {
                Schema::Object(object_schema) => {
                    format_table(output, list, object_schema, options)?;
                }
                _ => {
                    unimplemented!();
                }
            }
        }
    }
    Ok(())
}
