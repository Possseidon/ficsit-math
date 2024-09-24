use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ClassCollection<'a> {
    pub(crate) native_class: &'a str,
    pub(crate) classes: Vec<Class<'a>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Class<'a> {
    #[serde(rename = "ClassName")]
    pub(crate) name: &'a str,
    #[serde(flatten)]
    pub(crate) members: BTreeMap<&'a str, Value>,
}

impl Class<'_> {
    pub(crate) fn get_string(&self, member: &str) -> &str {
        self.members.get(member).expect(member).as_str().unwrap()
    }
}
