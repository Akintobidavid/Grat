pub mod auth;
pub mod auth_address_nonce;
pub mod auth_signature;
pub mod chain_analyzer;
pub mod context;
pub mod contract_error;
pub mod contract_error_resolver;
pub mod cross_contract;
pub mod decode_context;
pub mod diagnostic;
pub mod event_walker;
pub mod fee_analyzer;
pub mod function_call_decoder;
pub mod host_error;
pub mod mappings;
pub mod report;
pub mod resource_analyzer;
pub mod scval_to_json;
pub mod walker;

pub use auth::{
    AddressCredential, AuthChain, AuthCredential, AuthFunctionKind, AuthInvocation,
    AuthorizationType,
};
pub use auth_address_nonce::AddressWithNonce;
pub use chain_analyzer::{analyze_call_chain, CallChain, ChainAnalyzer, ChainFrame, FrameRole};
pub use resource_analyzer::{
    MetricDiagnostic, MetricKind, ResourceDiagnostics, ResourceMetrics, ResourceUsageAnalyzer,
    TransactionResultMeta,
};
pub use scval_to_json::scval_to_json;
pub use walker::{
    walk_diagnostic_events, DiagnosticEventKind, DiagnosticEventWalker, StructuredDiagnosticEvent,
};

// --- parse_v3_metadata and other functions remain unchanged ---
// (your full implementation of parse_v3_metadata, filter_transaction_by_operation,
// decode_transaction, decode_transaction_with_op_filter, and tests go here)
