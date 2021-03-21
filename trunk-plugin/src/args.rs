use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Args {
    pub user_arguments: Map<String, Value>,
}

impl Args {
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
