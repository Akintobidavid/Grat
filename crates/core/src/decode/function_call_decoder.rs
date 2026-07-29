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
