use serde::{Deserialize, Serialize};
use serde_json::Value;

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
