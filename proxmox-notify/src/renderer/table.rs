use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::ValueRenderFunction;

#[derive(Debug, Deserialize)]
pub struct ColumnSchema {
    pub label: String,
    pub id: String,
    pub renderer: Option<ValueRenderFunction>,
}

#[derive(Debug, Deserialize)]
pub struct TableSchema {
    pub columns: Vec<ColumnSchema>,
}

#[derive(Debug, Deserialize)]
pub struct Table {
    pub schema: TableSchema,
    pub data: Vec<HashMap<String, Value>>,
}
