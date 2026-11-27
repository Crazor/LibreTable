//! Rejections, as message IDs with arguments.

use std::collections::BTreeMap;
use std::fmt::Display;

use crate::protocol::ErrorMessage;

/// A rejected request: a `server-error-…` message ID, its arguments and
/// optionally a game's reason ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerError {
    code: &'static str,
    args: BTreeMap<String, String>,
    reason: Option<String>,
}

impl ServerError {
    /// An error with message ID `code`, which must be in
    /// [`MESSAGE_IDS`](crate::MESSAGE_IDS).
    #[must_use]
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            args: BTreeMap::new(),
            reason: None,
        }
    }

    /// Adds the argument `name`.
    #[must_use]
    pub fn arg(mut self, name: &str, value: impl Display) -> Self {
        self.args.insert(name.to_owned(), value.to_string());
        self
    }

    /// Adds a game's reason message ID.
    #[must_use]
    pub fn reason(mut self, id: &str) -> Self {
        self.reason = Some(id.to_owned());
        self
    }

    /// The message ID.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl From<ServerError> for ErrorMessage {
    fn from(error: ServerError) -> Self {
        Self {
            code: error.code.to_owned(),
            args: error.args,
            reason: error.reason,
        }
    }
}
