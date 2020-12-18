use std::io::Write;

use anyhow::*;
use serde_json::Value;

use crate::api::schema::*;

/// allows to configure the default output fromat using environment vars
pub const ENV_VAR_PROXMOX_OUTPUT_FORMAT: &str = "PROXMOX_OUTPUT_FORMAT";
/// if set, supress borders (and headers) when printing tables
pub const ENV_VAR_PROXMOX_OUTPUT_NO_BORDER: &str = "PROXMOX_OUTPUT_NO_BORDER";
/// if set, supress headers when printing tables
pub const ENV_VAR_PROXMOX_OUTPUT_NO_HEADER: &str = "PROXMOX_OUTPUT_NO_HEADER";

/// Helper to get output format from parameters or environment
pub fn get_output_format(param: &Value) -> String {
    let mut output_format = None;

    if let Some(format) = param["output-format"].as_str() {
        output_format = Some(format.to_owned());
    } else if let Ok(format) = std::env::var(ENV_VAR_PROXMOX_OUTPUT_FORMAT) {
        output_format = Some(format);
    }

    output_format.unwrap_or_else(|| String::from("text"))
}

/// Helper to get TableFormatOptions with default from environment
pub fn default_table_format_options() -> TableFormatOptions {
    let no_border = std::env::var(ENV_VAR_PROXMOX_OUTPUT_NO_BORDER)
        .ok()
        .is_some();
    let no_header = std::env::var(ENV_VAR_PROXMOX_OUTPUT_NO_HEADER)
        .ok()
        .is_some();

    TableFormatOptions::new()
        .noborder(no_border)
        .noheader(no_header)
}

/// Render function
///
/// Should convert the json `value` into a text string. `record` points to
/// the surrounding data object.
pub type RenderFunction =
    fn(/* value: */ &Value, /* record: */ &Value) -> Result<String, Error>;

fn data_to_text(data: &Value, schema: &Schema) -> Result<String, Error> {
    if data.is_null() {
        return Ok(String::new());
    }

    match schema {
        Schema::Null => {
            // makes no sense to display Null columns
            bail!("internal error");
        }
        Schema::Boolean(_) => match data.as_bool() {
            Some(value) => Ok(String::from(if value { "1" } else { "0" })),
            None => bail!("got unexpected data (expected bool)."),
        },
        Schema::Integer(_) => match data.as_i64() {
            Some(value) => Ok(format!("{}", value)),
            None => bail!("got unexpected data (expected integer)."),
        },
        Schema::Number(_) => match data.as_f64() {
            Some(value) => Ok(format!("{}", value)),
            None => bail!("got unexpected data (expected number)."),
        },
        Schema::String(_) => match data.as_str() {
            Some(value) => Ok(value.to_string()),
            None => bail!("got unexpected data (expected string)."),
        },
        Schema::Object(_) => Ok(data.to_string()),
        Schema::Array(_) => Ok(data.to_string()),
        Schema::AllOf(_) => Ok(data.to_string()),
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
    fn new<I>(column_widths: I, ascii_delimiters: bool) -> Self
    where
        I: Iterator<Item = usize>,
    {
        let mut top = String::new();
        let mut head = String::new();
        let mut middle = String::new();
        let mut bottom = String::new();

        let column_separator = if ascii_delimiters { '|' } else { '│' };

        for (i, column_width) in column_widths.enumerate() {
            if ascii_delimiters {
                top.push('+');
                head.push('+');
                middle.push('+');
                bottom.push('+');
            } else if i == 0 {
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

            for _j in 0..(column_width + 2) {
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
    pub header: Option<String>,
    pub right_align: Option<bool>,
    pub renderer: Option<RenderFunction>,
}

impl ColumnConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            header: None,
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

    pub fn header<S: Into<String>>(mut self, header: S) -> Self {
        self.header = Some(header.into());
        self
    }
}

/// Table formatter configuration
#[derive(Default)]
pub struct TableFormatOptions {
    /// Can be used to sort after a specific columns, if it isn't set
    /// we sort after the leftmost column (with no undef value in
    /// $data) this can be turned off by passing and empty array. The
    /// boolean argument specifies the sort order (false => ASC, true => DESC)
    pub sortkeys: Option<Vec<(String, bool)>>,
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
    /// This tests if stdout is a TTY and sets the columns to the terminal width,
    /// and sets ascii_delimiters to true If the locale CODESET is not UTF-8.
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

    pub fn disable_sort(mut self) -> Self {
        self.sortkeys = Some(Vec::new());
        self
    }

    pub fn sortby<S: Into<String>>(mut self, key: S, sort_desc: bool) -> Self {
        let key = key.into();
        match self.sortkeys {
            None => {
                let mut list = Vec::new();
                list.push((key, sort_desc));
                self.sortkeys = Some(list);
            }
            Some(ref mut list) => {
                list.push((key, sort_desc));
            }
        }
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

    fn lookup_column_info(
        &self,
        column_name: &str,
    ) -> (String, Option<bool>, Option<RenderFunction>) {
        let mut renderer = None;

        let header;
        let mut right_align = None;

        if let Some(column_config) = self.column_config.iter().find(|v| v.name == *column_name) {
            renderer = column_config.renderer;
            right_align = column_config.right_align;
            if let Some(ref h) = column_config.header {
                header = h.to_owned();
            } else {
                header = column_name.to_string();
            }
        } else {
            header = column_name.to_string();
        }

        (header, right_align, renderer)
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

fn format_table<W: Write, I: Iterator<Item = &'static SchemaPropertyEntry>>(
    output: W,
    list: &mut Vec<Value>,
    schema: &dyn ObjectSchemaType<PropertyIter = I>,
    options: &TableFormatOptions,
) -> Result<(), Error> {
    let properties_to_print = if options.column_config.is_empty() {
        extract_properties_to_print(schema.properties())
    } else {
        options
            .column_config
            .iter()
            .map(|v| v.name.clone())
            .collect()
    };

    let column_count = properties_to_print.len();
    if column_count == 0 {
        return Ok(());
    };

    let sortkeys = if let Some(ref sortkeys) = options.sortkeys {
        sortkeys.clone()
    } else {
        let mut keys = Vec::new();
        keys.push((properties_to_print[0].clone(), false)); // leftmost, ASC
        keys
    };

    let mut sortinfo = Vec::new();

    for (sortkey, sort_order) in sortkeys {
        let (_optional, sort_prop_schema) = match schema.lookup(&sortkey) {
            Some(tup) => tup,
            None => bail!("property {} does not exist in schema.", sortkey),
        };
        let numeric_sort = match sort_prop_schema {
            Schema::Integer(_) => true,
            Schema::Number(_) => true,
            _ => false,
        };
        sortinfo.push((sortkey, sort_order, numeric_sort));
    }

    use std::cmp::Ordering;
    list.sort_unstable_by(move |a, b| {
        for &(ref sortkey, sort_desc, numeric) in &sortinfo {
            let res = if numeric {
                let (v1, v2) = if sort_desc {
                    (b[&sortkey].as_f64(), a[&sortkey].as_f64())
                } else {
                    (a[&sortkey].as_f64(), b[&sortkey].as_f64())
                };
                match (v1, v2) {
                    (None, None) => Ordering::Greater,
                    (Some(_), None) => Ordering::Greater,
                    (None, Some(_)) => Ordering::Less,
                    (Some(a), Some(b)) =>
                    {
                        #[allow(clippy::if_same_then_else)]
                        if a.is_nan() {
                            Ordering::Greater
                        } else if b.is_nan() {
                            Ordering::Less
                        } else if a < b {
                            Ordering::Less
                        } else if a > b {
                            Ordering::Greater
                        } else {
                            Ordering::Equal
                        }
                    }
                }
            } else {
                let (v1, v2) = if sort_desc {
                    (b[sortkey].as_str(), a[sortkey].as_str())
                } else {
                    (a[sortkey].as_str(), b[sortkey].as_str())
                };
                v1.cmp(&v2)
            };

            if res != Ordering::Equal {
                return res;
            }
        }
        Ordering::Equal
    });

    let mut tabledata: Vec<TableColumn> = Vec::new();

    let mut column_names = Vec::new();

    for name in properties_to_print.iter() {
        let (_optional, prop_schema) = match schema.lookup(name) {
            Some(tup) => tup,
            None => bail!("property {} does not exist in schema.", name),
        };

        let is_numeric = match prop_schema {
            Schema::Integer(_) => true,
            Schema::Number(_) => true,
            Schema::Boolean(_) => true,
            _ => false,
        };

        let (header, right_align, renderer) = options.lookup_column_info(name);

        let right_align = right_align.unwrap_or(is_numeric);

        let mut max_width = if options.noheader || options.noborder {
            0
        } else {
            header.chars().count()
        };

        column_names.push(header);

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

            let lines: Vec<String> = text
                .lines()
                .map(|line| {
                    let width = line.chars().count();
                    if width > max_width {
                        max_width = width;
                    }
                    line.to_string()
                })
                .collect();

            cells.push(TableCell { lines });
        }

        tabledata.push(TableColumn {
            cells,
            width: max_width,
            right_align,
        });
    }

    render_table(output, &tabledata, &column_names, options)
}

fn render_table<W: Write>(
    mut output: W,
    tabledata: &[TableColumn],
    column_names: &[String],
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

    let column_widths = tabledata.iter().map(|d| d.width);
    let borders = TableBorders::new(column_widths, options.ascii_delimiters);

    if !options.noborder {
        write_line(&borders.top)?;
    }

    let mut header = String::new();
    for (i, name) in column_names.iter().enumerate() {
        let column = &tabledata[i];
        header.push(borders.column_separator);
        header.push(' ');
        if column.right_align {
            header.push_str(&format!("{:>width$}", name, width = column.width));
        } else {
            header.push_str(&format!("{:<width$}", name, width = column.width));
        }
        header.push(' ');
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
            if lines.len() > max_lines {
                max_lines = lines.len();
            }
        }
        for line_nr in 0..max_lines {
            let mut text = String::new();
            let empty_string = String::new();
            for (i, _name) in column_names.iter().enumerate() {
                let column = &tabledata[i];
                let lines = &column.cells[pos].lines;
                let line = lines.get(line_nr).unwrap_or(&empty_string);

                if options.noborder {
                    if i > 0 {
                        text.push(' ');
                    }
                } else {
                    text.push(borders.column_separator);
                    text.push(' ');
                }

                if column.right_align {
                    text.push_str(&format!("{:>width$}", line, width = column.width));
                } else {
                    text.push_str(&format!("{:<width$}", line, width = column.width));
                }

                if !options.noborder {
                    text.push(' ');
                }
            }
            if !options.noborder {
                text.push(borders.column_separator);
            }
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

fn format_object<W: Write, I: Iterator<Item = &'static SchemaPropertyEntry>>(
    output: W,
    data: &Value,
    schema: &dyn ObjectSchemaType<PropertyIter = I>,
    options: &TableFormatOptions,
) -> Result<(), Error> {
    let properties_to_print = if options.column_config.is_empty() {
        extract_properties_to_print(schema.properties())
    } else {
        options
            .column_config
            .iter()
            .map(|v| v.name.clone())
            .collect()
    };

    let row_count = properties_to_print.len();
    if row_count == 0 {
        return Ok(());
    };

    const NAME_TITLE: &str = "Name";
    const VALUE_TITLE: &str = "Value";

    let mut max_name_width = if options.noheader || options.noborder {
        0
    } else {
        NAME_TITLE.len()
    };
    let mut max_value_width = if options.noheader || options.noborder {
        0
    } else {
        VALUE_TITLE.len()
    };

    let column_names = vec![NAME_TITLE.to_string(), VALUE_TITLE.to_string()];

    let mut name_cells = Vec::new();
    let mut value_cells = Vec::new();

    let mut all_right_aligned = true;

    for name in properties_to_print.iter() {
        let (optional, prop_schema) = match schema.lookup(name) {
            Some(tup) => tup,
            None => bail!("property {} does not exist in schema.", name),
        };

        let is_numeric = match prop_schema {
            Schema::Integer(_) => true,
            Schema::Number(_) => true,
            Schema::Boolean(_) => true,
            _ => false,
        };

        let (header, right_align, renderer) = options.lookup_column_info(name);

        let right_align = right_align.unwrap_or(is_numeric);

        if !right_align {
            all_right_aligned = false;
        }

        if optional {
            if let Some(object) = data.as_object() {
                if object.get(name).is_none() {
                    continue;
                }
            }
        }

        let header_width = header.chars().count();
        if header_width > max_name_width {
            max_name_width = header_width;
        }

        name_cells.push(TableCell {
            lines: vec![header],
        });

        let result = if let Some(renderer) = renderer {
            (renderer)(&data[name], &data)
        } else {
            data_to_text(&data[name], prop_schema)
        };

        let text = match result {
            Ok(text) => text,
            Err(err) => bail!("unable to format property {} - {}", name, err),
        };

        let lines: Vec<String> = text
            .lines()
            .map(|line| {
                let width = line.chars().count();
                if width > max_value_width {
                    max_value_width = width;
                }
                line.to_string()
            })
            .collect();

        value_cells.push(TableCell { lines });
    }

    let name_column = TableColumn {
        cells: name_cells,
        width: max_name_width,
        right_align: false,
    };
    let value_column = TableColumn {
        cells: value_cells,
        width: max_value_width,
        right_align: all_right_aligned,
    };

    let mut tabledata: Vec<TableColumn> = Vec::new();
    tabledata.push(name_column);
    tabledata.push(value_column);

    render_table(output, &tabledata, &column_names, options)
}

fn extract_properties_to_print<I>(properties: I) -> Vec<String>
where
    I: Iterator<Item = &'static SchemaPropertyEntry>,
{
    let mut result = Vec::new();
    let mut opt_properties = Vec::new();

    for (name, optional, _prop_schema) in properties {
        if *optional {
            opt_properties.push(name.to_string());
        } else {
            result.push(name.to_string());
        }
    }

    result.extend(opt_properties);

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
            if list.is_empty() {
                return Ok(());
            }

            match array_schema.items {
                Schema::Object(object_schema) => {
                    format_table(output, list, object_schema, options)?;
                }
                Schema::AllOf(all_of_schema) => {
                    format_table(output, list, all_of_schema, options)?;
                }
                _ => {
                    unimplemented!();
                }
            }
        }
        Schema::AllOf(all_of_schema) => {
            format_object(output, data, all_of_schema, options)?;
        }
    }
    Ok(())
}
