/// Options for `DocHandle.registerFont`.
#[napi(object)]
pub struct RegisterFontOptions {
    pub family_name: Option<String>,
    pub weight: Option<String>,
    pub style: Option<String>,
    pub stretch: Option<String>,
}
