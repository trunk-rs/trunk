#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Output {
    pub msg: String,
}

impl Output {
    #[cfg(feature = "runtime")]
    pub fn from_wasm_ret(mem: &wasmer_runtime::Memory, ptr: u32, len: u32) -> crate::Result<Self> {
        use crate::error::Error;
        use std::cell::Cell;

        let buf = mem
            .view()
            .get(ptr as usize..(ptr + len) as usize)
            .ok_or(Error::InvalidMemorySlice { ptr, len })?
            .iter()
            .map(Cell::get)
            .collect::<Vec<u8>>();

        Ok(serde_cbor::from_slice(&buf)?)
    }
}

impl From<()> for Output {
    fn from(_: ()) -> Self {
        Self::default()
    }
}
