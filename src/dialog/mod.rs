//! Native file dialogs on Windows via `rfd::AsyncFileDialog`.
//!
//! All methods are async napi functions so the JS event loop is not
//! blocked while the native dialog is open.

/// Extension filter entry, e.g. `{ name: "Images", extensions: ["png", "jpg"] }`.
#[napi(object)]
pub struct FileFilter {
    /// Display name shown in the filter dropdown.
    pub name: String,
    /// File extensions without leading dot, e.g. `["png", "jpg"]`.
    pub extensions: Vec<String>,
}

/// Options shared by all dialog methods.
#[napi(object)]
pub struct DialogOptions {
    /// Dialog title.
    pub title: Option<String>,
    /// Starting directory.
    pub directory: Option<String>,
    /// Starting file name (save dialog) or default name.
    pub file_name: Option<String>,
    /// Extension filters.
    pub filters: Option<Vec<FileFilter>>,
}

fn build_dialog(opts: Option<&DialogOptions>) -> rfd::AsyncFileDialog {
    let mut d = rfd::AsyncFileDialog::new();
    if let Some(o) = opts {
        if let Some(t) = &o.title {
            d = d.set_title(t);
        }
        if let Some(dir) = &o.directory {
            d = d.set_directory(dir);
        }
        if let Some(name) = &o.file_name {
            d = d.set_file_name(name);
        }
        if let Some(filters) = &o.filters {
            for f in filters {
                let exts: Vec<&str> = f.extensions.iter().map(|s| s.as_str()).collect();
                d = d.add_filter(&f.name, &exts);
            }
        }
    }
    d
}

/// Open a single-file picker. Returns the chosen path or `null`.
#[napi]
pub async fn pick_file(options: Option<DialogOptions>) -> Option<String> {
    let d = build_dialog(options.as_ref());
    d.pick_file()
        .await
        .map(|h| h.path().to_string_lossy().into_owned())
}

/// Open a multi-file picker. Returns an array of paths (may be empty).
#[napi]
pub async fn pick_files(options: Option<DialogOptions>) -> Vec<String> {
    let d = build_dialog(options.as_ref());
    let handles = d.pick_files().await;
    handles
        .into_iter()
        .flatten()
        .map(|h| h.path().to_string_lossy().into_owned())
        .collect()
}

/// Open a single-folder picker. Returns the chosen path or `null`.
#[napi]
pub async fn pick_folder(options: Option<DialogOptions>) -> Option<String> {
    let d = build_dialog(options.as_ref());
    d.pick_folder()
        .await
        .map(|h| h.path().to_string_lossy().into_owned())
}

/// Open a multi-folder picker. Returns an array of paths (may be empty).
#[napi]
pub async fn pick_folders(options: Option<DialogOptions>) -> Vec<String> {
    let d = build_dialog(options.as_ref());
    let handles = d.pick_folders().await;
    handles
        .into_iter()
        .flatten()
        .map(|h| h.path().to_string_lossy().into_owned())
        .collect()
}

/// Open a save-file dialog. Returns the chosen path or `null`.
#[napi]
pub async fn save_file(options: Option<DialogOptions>) -> Option<String> {
    let d = build_dialog(options.as_ref());
    d.save_file()
        .await
        .map(|h| h.path().to_string_lossy().into_owned())
}
