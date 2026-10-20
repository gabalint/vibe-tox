//! Report windows: a stored PDF the engine returned after its checks, shown by the web
//! view's own PDF viewer in a window of its own (REQ-OPS-015, proposed).
//!
//! Invariants:
//! - The bytes stay in memory for the window's life and are served only to the window
//!   they were opened for, through the [`SCHEME`] protocol; nothing is written to disk.
//! - A report window has no capability, so it cannot call any command (NFR-31), and it
//!   never navigates away from its report, so a link in a PDF opens nothing.
//! - Not every platform's web view displays PDFs (WebKitGTK depends on its version), so
//!   the PDF can always be saved as well.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use lis_core::{CommandError, CorrelationId, ErrorCode};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{
    AppHandle, Manager, Runtime, UriSchemeContext, WebviewUrl, WebviewWindowBuilder, Window,
};

/// The protocol serving each report window its PDF. WebView2 reaches it as
/// `http://lis-report.localhost/`, which wry maps back.
pub(crate) const SCHEME: &str = "lis-report";

/// The PDFs of the open report windows, by window label.
#[derive(Debug, Default)]
pub(crate) struct ReportWindows(Mutex<HashMap<String, Vec<u8>>>);

impl ReportWindows {
    fn get(&self, label: &str) -> Option<Vec<u8>> {
        self.lock().get(label).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<u8>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Shows `pdf` in the report window `label`, opening it or bringing it to the front.
pub(crate) fn show<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    title: &str,
    pdf: Vec<u8>,
) -> Result<(), CommandError> {
    let windows = app.state::<ReportWindows>();
    windows.lock().insert(label.to_owned(), pdf);
    let shown = match app.get_webview_window(label) {
        Some(window) => window.unminimize().and_then(|()| window.set_focus()),
        None => open(app, label, title),
    };
    shown.map_err(|error| {
        windows.lock().remove(label);
        let correlation_id = CorrelationId::generate();
        tracing::error!(%correlation_id, %error, "report window not shown");
        CommandError::new(
            ErrorCode::Internal,
            "The report window could not be opened. Save the PDF instead, or give support \
             the reference shown.",
            correlation_id,
        )
    })
}

fn open<R: Runtime>(app: &AppHandle<R>, label: &str, title: &str) -> tauri::Result<()> {
    let url = format!("{SCHEME}://localhost/{label}.pdf");
    let url = url
        .parse()
        .map_err(|_| tauri::Error::InvalidWebviewUrl("report window"))?;
    WebviewWindowBuilder::new(app, label, WebviewUrl::CustomProtocol(url))
        .title(title)
        .inner_size(900.0, 760.0)
        .min_inner_size(480.0, 360.0)
        .on_navigation(|url| {
            url.scheme() == SCHEME || url.host_str() == Some(&format!("{SCHEME}.localhost"))
        })
        .build()
        .map(drop)
}

/// The [`SCHEME`] protocol: the requesting window's own PDF, or 404.
pub(crate) fn serve<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    _request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let pdf = ctx
        .app_handle()
        .try_state::<ReportWindows>()
        .and_then(|windows| windows.get(ctx.webview_label()));
    let response = match pdf {
        Some(pdf) => Response::builder()
            .header(header::CONTENT_TYPE, "application/pdf")
            .body(Cow::Owned(pdf)),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Cow::Borrowed(&[][..])),
    };
    response.unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

/// Drops a closed report window's PDF.
pub(crate) fn forget<R: Runtime>(window: &Window<R>) {
    if let Some(windows) = window.try_state::<ReportWindows>() {
        windows.lock().remove(window.label());
    }
}
