use pcsc::{Context, Scope, Error, ReaderState, State};
use std::ffi::CString;
use std::time::Duration;

/// Establish a PC/SC context in user scope.
pub fn establish() -> Result<Context, Error> {
    Context::establish(Scope::User)
}

/// List all connected readers.
pub fn list_readers(ctx: &Context) -> Result<Vec<String>, Error> {
    let mut buf = vec![0u8; 4096];
    let names = ctx.list_readers(&mut buf)?;
    Ok(names
        .map(|n| n.to_str().unwrap_or("???").to_string())
        .collect())
}

/// Poll a specific reader for card presence. Returns `true` if card is present.
/// Non-blocking: uses 500ms timeout.
pub fn poll_card_present(ctx: &Context, reader_name: &str) -> Result<bool, Error> {
    let name_cstr = CString::new(reader_name.as_bytes()).expect("reader name contains null");
    let rs = ReaderState::new(name_cstr, State::UNAWARE);
    let mut states = [rs];
    match ctx.get_status_change(Duration::from_millis(500), &mut states) {
        Ok(()) => Ok(states[0].event_state().contains(State::PRESENT)),
        Err(Error::Timeout) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn establish_context() {
        let ctx = establish();
        assert!(ctx.is_ok(), "PC/SC context should establish on systems with PC/SC");
    }
}
