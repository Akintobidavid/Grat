use serde::Serialize;
use serde_json::Value;

pub type JsonValue = Value;

#[derive(Debug, Clone, Serialize)]
pub struct DecodedArgument {
    pub name: String,

    pub value: JsonValue,

    pub formatted: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecodedFunctionCall {
    pub function_name: String,

    pub arguments: Vec<DecodedArgument>,

    pub return_value: Option<JsonValue>,

    pub formatted_return_value: Option<String>,
}

pub struct FunctionCallDecoder;

impl FunctionCallDecoder {
    pub fn new() -> Self {
        Self
    }
}
