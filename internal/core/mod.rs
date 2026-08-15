//! Domain placeholder.
//!
//! Workhorse templates ship no product features. Replace this module during
//! CDRL with application services, models, and handlers.

/// Echo a message. Placeholder used by the HTTP `/echo` route.
pub fn echo(message: &str) -> String {
    message.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_returns_input() {
        assert_eq!(echo("ping"), "ping");
    }
}
