//! The `MessageEvent` class — inherits from `Event`.

use napi::Result;
use napi_helpers::any_value::AnyValue;
use napi_inherit_proc::layer;

/// Own block of the `MessageEvent` class.
#[layer(js_name = "MessageEvent", parent = super::EventLayer)]
pub struct MessageEventLayer {
    data: AnyValue,
    origin: String,
}

#[layer]
impl MessageEventLayer {
    /// `new MessageEvent(type, data, origin)`.
    #[layer(constructor)]
    fn build(data: Option<AnyValue>, origin: Option<String>) -> Result<Self> {
        Ok(Self {
            data: data.unwrap_or(AnyValue::Null),
            origin: origin.unwrap_or_default(),
        })
    }

    #[layer(getter)]
    fn data(&self) -> AnyValue {
        self.data.clone()
    }

    #[layer(getter)]
    fn origin(&self) -> String {
        self.origin.clone()
    }
}
