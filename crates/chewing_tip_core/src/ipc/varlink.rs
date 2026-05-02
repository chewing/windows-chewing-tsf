use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{impl_context_error, result::expect_error};

#[derive(Serialize, Deserialize, Debug)]
pub struct MethodCall {
    pub method: String,
    pub parameters: Value,
    pub oneway: Option<bool>,
    pub more: Option<bool>,
    pub upgrade: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MethodReply {
    pub parameters: Value,
    pub continues: Option<bool>,
    pub error: Option<String>,
}

impl MethodReply {
    pub fn to_bytes(&self) -> Result<Vec<u8>, VarLinkError> {
        expect_error("Failed to serialize varlink MethodReply", || {
            let mut buf = serde_json::to_vec(self)?;
            buf.push(0);
            Ok(buf)
        })
    }
}

impl_context_error!(pub VarLinkError);
