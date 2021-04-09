use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::Permissions;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Args {
    pub user_arguments: Map<String, Value>,
    pub permissions: Permissions,
}

impl Args {
    const PLUGIN_ARGS_ATTR: &'static str = "data-args";
    const PLUGIN_ARG_ATTR_PREFIX: &'static str = "data-";

    pub fn from_link_attrs(mut link_attrs: HashMap<String, String>) -> serde_json::Result<Self> {
        let mut user_arguments = link_attrs
            .remove(Self::PLUGIN_ARGS_ATTR)
            .as_deref()
            .map(serde_json::from_str::<Map<String, Value>>)
            .unwrap_or_else(|| Ok(Map::new()))?;

        link_attrs
            .drain()
            .filter(|(k, _)| k.starts_with(Self::PLUGIN_ARG_ATTR_PREFIX))
            .try_for_each(|(k, v)| {
                let v = serde_json::from_str::<Value>(&v)?;
                user_arguments.insert(k, v);
                Ok(())
            })?;

        Ok(Self {
            user_arguments,
            permissions: Permissions::NONE,
        })
    }

    pub fn with_permissions(mut self, permissions: Permissions) -> Self {
        self.permissions.insert(permissions);
        self
    }

    pub fn parse_user_arguments<T>(self) -> serde_json::Result<T>
        where
            T: serde::de::DeserializeOwned,
    {
        serde_json::from_value(Value::Object(self.user_arguments))
    }

    #[cfg(feature = "runtime")]
    pub fn call_main(&self, instance: &wasmer_runtime::Instance) -> crate::Result<crate::Output> {
        let mem = instance.context().memory(0);
        let (arg_ptr, arg_len) = self.as_wasm_args(mem);

        let func: wasmer_runtime::Func<(u32, u32), (u32, u32)> = instance.exports.get("main").map_err(wasmer_runtime::error::Error::from)?;
        let (out_ptr, out_len) = func.call(arg_ptr, arg_len).map_err(wasmer_runtime::error::Error::from)?;

        crate::Output::from_wasm_ret(mem, out_ptr, out_len)
    }

    #[cfg(feature = "runtime")]
    pub fn as_wasm_args(&self, mem: &wasmer_runtime::Memory) -> (u32, u32) {
        let buf = serde_cbor::to_vec(self).expect("Serializing Args into a Vec will never fail");
        let buf_len = buf.len();

        mem.view()[..buf_len].iter().zip(buf).for_each(|(cell, byte)| cell.set(byte));

        (0, buf_len as u32)
    }
}
