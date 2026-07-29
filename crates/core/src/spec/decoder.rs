use crate::error::{GratError, GratResult};
use serde::{Deserialize, Serialize};
use stellar_xdr::curr::{Limited, Limits, ReadXdr, ScSpecEntry, ScSpecTypeDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractErrorEntry {
    pub code: u32,

    pub name: String,

    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractFunction {
    pub name: String,

    pub params: Vec<(String, String)>,

    pub return_type: String,

    pub doc: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub return_type_def: Option<ScSpecTypeDef>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub param_defs: Vec<(String, ScSpecTypeDef)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractStructField {
    pub name: String,

    pub type_name: String,

    pub doc: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub type_def: Option<ScSpecTypeDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractStructDef {
    pub name: String,

    pub fields: Vec<ContractStructField>,

    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEnumCase {
    pub name: String,

    pub value: u32,

    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEnumDef {
    pub name: String,

    pub cases: Vec<ContractEnumCase>,

    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractUnionCase {
    pub name: String,

    pub doc: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub value_types: Option<Vec<ScSpecTypeDef>>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fields: Option<Vec<ContractStructField>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractUnionDef {
    pub name: String,

    pub cases: Vec<ContractUnionCase>,

    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSpec {
    pub errors: Vec<ContractErrorEntry>,

    pub functions: Vec<ContractFunction>,

    pub structs: Vec<ContractStructDef>,

    pub name: Option<String>,

    pub version: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub enums: Vec<ContractEnumDef>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unions: Vec<ContractUnionDef>,
}

pub fn decode_contract_spec(wasm_bytes: &[u8]) -> GratResult<ContractSpec> {
    let raw_spec = SpecParser::extract_spec(wasm_bytes)?;

    let mut errors = Vec::new();
    let mut functions = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut unions = Vec::new();
    let name = None;
    let version = None;

    let mut cursor = std::io::Cursor::new(&raw_spec);
    let mut limited = Limited::new(&mut cursor, Limits::none());
    while let Ok(entry) = ScSpecEntry::read_xdr(&mut limited) {
        match entry {
            ScSpecEntry::FunctionV0(func) => {
                let func_name = func.name.to_string();
                let doc = if func.doc.is_empty() {
                    None
                } else {
                    Some(func.doc.to_string())
                };

                let mut params = Vec::new();
                let mut param_defs = Vec::new();
                for input in func.inputs.iter() {
                    let param_name = input.name.to_string();
                    let param_type = format_type_def(&input.type_);
                    params.push((param_name.clone(), param_type));
                    param_defs.push((param_name, input.type_.clone()));
                }

                let return_type_def = if func.outputs.is_empty() {
                    Some(ScSpecTypeDef::Void)
                } else {
                    Some(func.outputs[0].clone())
                };

                let return_type = if func.outputs.is_empty() {
                    "Void".to_string()
                } else {
                    format_type_def(&func.outputs[0])
                };

                functions.push(ContractFunction {
                    name: func_name,
                    params,
                    return_type,
                    doc,
                    return_type_def,
                    param_defs,
                });
            }
            ScSpecEntry::UdtErrorEnumV0(err_enum) => {
                let enum_name = err_enum.name.to_string();
                for case in err_enum.cases.iter() {
                    let case_name = format!("{}::{}", enum_name, case.name);
                    let doc = if case.doc.is_empty() {
                        None
                    } else {
                        Some(case.doc.to_string())
                    };

                    errors.push(ContractErrorEntry {
                        code: case.value,
                        name: case_name,
                        doc,
                    });
                }
            }
            ScSpecEntry::UdtEnumV0(enum_spec) => {
                let enum_name = enum_spec.name.to_string();
                let doc = if enum_spec.doc.is_empty() {
                    None
                } else {
                    Some(enum_spec.doc.to_string())
                };
                let mut cases = Vec::new();
                for case in enum_spec.cases.iter() {
                    let case_doc = if case.doc.is_empty() {
                        None
                    } else {
                        Some(case.doc.to_string())
                    };
                    cases.push(ContractEnumCase {
                        name: case.name.to_string(),
                        value: case.value,
                        doc: case_doc,
                    });
                }
                enums.push(ContractEnumDef {
                    name: enum_name,
                    cases,
                    doc,
                });
            }
            ScSpecEntry::UdtUnionV0(union_spec) => {
                let union_name = union_spec.name.to_string();
                let doc = if union_spec.doc.is_empty() {
                    None
                } else {
                    Some(union_spec.doc.to_string())
                };
                let mut cases = Vec::new();
                for case in union_spec.cases.iter() {
                    match case {
                        stellar_xdr::curr::ScSpecUdtUnionCaseV0::VoidV0(c) => {
                            let case_doc = if c.doc.is_empty() {
                                None
                            } else {
                                Some(c.doc.to_string())
                            };
                            cases.push(ContractUnionCase {
                                name: c.name.to_string(),
                                doc: case_doc,
                                value_types: None,
                                fields: None,
                            });
                        }
                        stellar_xdr::curr::ScSpecUdtUnionCaseV0::TupleV0(c) => {
                            let case_doc = if c.doc.is_empty() {
                                None
                            } else {
                                Some(c.doc.to_string())
                            };
                            let value_types: Vec<ScSpecTypeDef> =
                                c.type_.iter().cloned().collect();
                            cases.push(ContractUnionCase {
                                name: c.name.to_string(),
                                doc: case_doc,
                                value_types: Some(value_types),
                                fields: None,
                            });
                        }

                    }
                }
                unions.push(ContractUnionDef {
                    name: union_name,
                    cases,
                    doc,
                });
            }
            ScSpecEntry::UdtStructV0(struct_spec) => {
                let struct_name = struct_spec.name.to_string();
                let doc = if struct_spec.doc.is_empty() {
                    None
                } else {
                    Some(struct_spec.doc.to_string())
                };

                let mut fields = Vec::new();
                for field in struct_spec.fields.iter() {
                    let field_name = field.name.to_string();
                    let field_type = format_type_def(&field.type_);
                    let field_doc = if field.doc.is_empty() {
                        None
                    } else {
                        Some(field.doc.to_string())
                    };
                    fields.push(ContractStructField {
                        name: field_name,
                        type_name: field_type,
                        doc: field_doc,
                        type_def: Some(field.type_.clone()),
                    });
                }

                structs.push(ContractStructDef {
                    name: struct_name,
                    fields,
                    doc,
                });
            }
        }
    }

    Ok(ContractSpec {
        errors,
        functions,
        structs,
        name,
        version,
        enums,
        unions,
    })
}

fn format_type_def(type_def: &ScSpecTypeDef) -> String {
    match type_def {
        ScSpecTypeDef::Val => "Val".to_string(),
        ScSpecTypeDef::Bool => "Bool".to_string(),
        ScSpecTypeDef::Void => "Void".to_string(),
        ScSpecTypeDef::Error => "Error".to_string(),
        ScSpecTypeDef::U32 => "U32".to_string(),
        ScSpecTypeDef::I32 => "I32".to_string(),
        ScSpecTypeDef::U64 => "U64".to_string(),
        ScSpecTypeDef::I64 => "I64".to_string(),
        ScSpecTypeDef::Timepoint => "Timepoint".to_string(),
        ScSpecTypeDef::Duration => "Duration".to_string(),
        ScSpecTypeDef::U128 => "U128".to_string(),
        ScSpecTypeDef::I128 => "I128".to_string(),
        ScSpecTypeDef::U256 => "U256".to_string(),
        ScSpecTypeDef::I256 => "I256".to_string(),
        ScSpecTypeDef::Bytes => "Bytes".to_string(),
        ScSpecTypeDef::BytesN(b) => format!("BytesN<{}>", b.n),
        ScSpecTypeDef::String => "String".to_string(),
        ScSpecTypeDef::Symbol => "Symbol".to_string(),
        ScSpecTypeDef::Address => "Address".to_string(),
        ScSpecTypeDef::Option(opt) => format!("Option<{}>", format_type_def(&opt.value_type)),
        ScSpecTypeDef::Result(res) => format!(
            "Result<{}, {}>",
            format_type_def(&res.ok_type),
            format_type_def(&res.error_type)
        ),
        ScSpecTypeDef::Vec(vec) => format!("Vec<{}>", format_type_def(&vec.element_type)),
        ScSpecTypeDef::Map(map) => format!(
            "Map<{}, {}>",
            format_type_def(&map.key_type),
            format_type_def(&map.value_type)
        ),
        ScSpecTypeDef::Tuple(tuple) => {
            let elements: Vec<String> = tuple.value_types.iter().map(format_type_def).collect();
            format!("({})", elements.join(", "))
        }
        ScSpecTypeDef::Udt(udt) => udt.name.to_string(),
    }
}

pub struct SpecParser;

impl SpecParser {
    pub fn extract_spec(wasm_bytes: &[u8]) -> GratResult<Vec<u8>> {
        let parser = wasmparser::Parser::new(0);
        for payload in parser.parse_all(wasm_bytes) {
            let payload =
                payload.map_err(|e| GratError::SpecError(format!("WASM parse error: {e}")))?;

            if let wasmparser::Payload::CustomSection(section) = payload {
                if section.name() == "contractspecv0" {
                    return Ok(section.data().to_vec());
                }
            }
        }

        Err(GratError::SpecError(
            "contractspecv0 custom section not found".into(),
        ))
    }
}

pub fn resolve_error_code(spec: &ContractSpec, error_code: u32) -> Option<&ContractErrorEntry> {
    spec.errors.iter().find(|e| e.code == error_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_error_code_not_found() {
        let spec = ContractSpec {
            errors: vec![ContractErrorEntry {
                code: 1,
                name: "NotFound".to_string(),
                doc: None,
            }],
            functions: Vec::new(),
            structs: Vec::new(),
            name: None,
            version: None,
            enums: Vec::new(),
            unions: Vec::new(),
        };
        assert!(resolve_error_code(&spec, 99).is_none());
        assert!(resolve_error_code(&spec, 1).is_some());
    }

    #[test]
    fn test_extract_spec_success() {
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let section_name = "contractspecv0";
        let section_data = vec![1, 2, 3, 4];

        let mut custom_payload = Vec::new();
        custom_payload.push(section_name.len() as u8);
        custom_payload.extend_from_slice(section_name.as_bytes());
        custom_payload.extend_from_slice(&section_data);

        wasm.push(0);
        wasm.push(custom_payload.len() as u8);
        wasm.extend(custom_payload);

        let result = SpecParser::extract_spec(&wasm).expect("Should find section");
        assert_eq!(result, section_data);
    }

    #[test]
    fn test_extract_spec_not_found() {
        let wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let result = SpecParser::extract_spec(&wasm);
        assert!(result.is_err());
        match result {
            Err(GratError::SpecError(msg)) => assert!(msg.contains("not found")),
            _ => panic!("Expected SpecError"),
        }
    }
}
