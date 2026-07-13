//! Error responses, localized and HTMX-aware.

use askama::Template;
use askama_web::WebTemplate;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};

use crate::i18n::{Lang, Tr};

/// Rejection of [`crate::web::session::SessionDataset`]: no uploaded dataset
/// (expired, evicted, or the server restarted).
pub struct SessionExpired {
    pub lang: Lang,
    pub is_htmx: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "partials/session_expired.html")]
struct SessionExpiredFragment {
    t: Tr,
}

impl IntoResponse for SessionExpired {
    fn into_response(self) -> Response {
        if self.is_htmx {
            // Rendered into whatever panel triggered the request; the link
            // inside leads back to the upload page.
            let fragment = SessionExpiredFragment {
                t: Tr::new(self.lang),
            };
            (StatusCode::UNPROCESSABLE_ENTITY, fragment).into_response()
        } else {
            Redirect::to("/?expired=1").into_response()
        }
    }
}
