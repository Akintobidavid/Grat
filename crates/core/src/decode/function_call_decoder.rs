use crate::decode::return_decoder::ReturnValueDecoder;
use crate::spec::decoder::{ContractFunction, ContractSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellar_xdr::curr::ScVal;

/// A fully decoded representation of a Soroban contract function invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedFunctionCall {
    pub function_name: String,

    pub arguments: Vec<Value>,

    pub formatted_arguments: Vec<String>,

    pub return_value: Option<Value>,

    pub formatted_return_value: Option<String>,
}

/// Decoder for contract function calls, handling argument list decoding
/// and delegating return value decoding to `ReturnValueDecoder`.
#[derive(Debug, Clone, Default)]
pub struct FunctionCallDecoder {
    return_decoder: ReturnValueDecoder,
}

impl FunctionCallDecoder {
    pub fn new() -> Self {
        Self {
            return_decoder: ReturnValueDecoder::new(),
        }
    }

    /// Decodes function call arguments into a vector of typed JSON values.
    pub fn decode_call_arguments(
        &self,
        args: &[ScVal],
        func: &ContractFunction,
        contract_spec: Option<&ContractSpec>,
    ) -> Vec<Value> {
        args.iter()
            .enumerate()
            .map(|(i, arg)| {
                let type_def = func.param_defs.get(i).map(|(_, td)| td);
                self.return_decoder.decode(arg, type_def, contract_spec)
            })
            .collect()
    }

    /// Decodes a function return value using the function's return specification.
    pub fn decode_return_value(
        &self,
        return_val: &ScVal,
        func: &ContractFunction,
        contract_spec: Option<&ContractSpec>,
    ) -> Value {
        self.return_decoder
            .decode_function_return(return_val, func, contract_spec)
    }

    /// Decodes a full function call (name, arguments, return value) using a `ContractSpec`.
    pub fn decode_function_call(
        &self,
        func_name: &str,
        args: &[ScVal],
        return_val: Option<&ScVal>,
        contract_spec: &ContractSpec,
    ) -> DecodedFunctionCall {
        let matching_func = contract_spec.functions.iter().find(|f| f.name == func_name);

        let (arguments, formatted_arguments) = if let Some(func) = matching_func {
            let decoded_args = self.decode_call_arguments(args, func, Some(contract_spec));
            let formatted: Vec<String> = decoded_args
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
                })
                .collect();
            (decoded_args, formatted)
        } else {
            let decoded_args: Vec<Value> = args.iter().map(ReturnValueDecoder::decode_dynamic).collect();
            let formatted: Vec<String> = decoded_args
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
                })
                .collect();
            (decoded_args, formatted)
        };

        let (return_value, formatted_return_value) = match return_val {
            Some(rv) => {
                let val = if let Some(func) = matching_func {
                    self.decode_return_value(rv, func, Some(contract_spec))
                } else {
                    ReturnValueDecoder::decode_dynamic(rv)
                };
                let formatted = match &val {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
                };
                (Some(val), Some(formatted))
            }
            None => (None, None),
        };

        DecodedFunctionCall {
            function_name: func_name.to_string(),
            arguments,
            formatted_arguments,
            return_value,
            formatted_return_value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::curr::{ScSpecTypeDef, ScSymbol};

    #[test]
    fn test_decode_function_call() {
        let decoder = FunctionCallDecoder::new();

        let func = ContractFunction {
            name: "transfer".to_string(),
            params: vec![
                ("to".to_string(), "Address".to_string()),
                ("amount".to_string(), "I128".to_string()),
            ],
            return_type: "Bool".to_string(),
            doc: None,
            return_type_def: Some(ScSpecTypeDef::Bool),
            param_defs: vec![
                ("to".to_string(), ScSpecTypeDef::Address),
                ("amount".to_string(), ScSpecTypeDef::I128),
            ],
        };

        let spec = ContractSpec {
            errors: vec![],
            functions: vec![func],
            structs: vec![],
            name: None,
            version: None,
            enums: vec![],
            unions: vec![],
        };

        let args = vec![
            ScVal::Symbol(ScSymbol("recipient".try_into().unwrap())),
            ScVal::I32(500),
        ];
        let return_val = ScVal::Bool(true);

        let decoded = decoder.decode_function_call("transfer", &args, Some(&return_val), &spec);
        assert_eq!(decoded.function_name, "transfer");
        assert_eq!(decoded.arguments.len(), 2);
        assert_eq!(decoded.return_value, Some(serde_json::json!(true)));
        assert_eq!(decoded.formatted_return_value, Some("true".to_string()));
    }
}
