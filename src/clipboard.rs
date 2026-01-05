use std::env;

use copypasta_ext::{prelude::*, x11_fork::ClipboardContext};

// Stolen from:
//   https://github.com/jakewilliami/cb/blob/d101beba/src/main.rs#L116-L148
pub fn copy(s: &str) {
    // Try set clipboard for WSL or SSH first, falling back to `clipboard` if unavailable
    let set_res = clipboard_anywhere::set_clipboard(s);
    let get_res = clipboard_anywhere::get_clipboard();

    // Possible errors:
    //   1. Something has gone wrong if we can neither set nor get the clipboard
    let clipboard_unresponsive = set_res.is_err() && get_res.is_err();
    //   2. If we are not using SSH, get_res should be okay
    let local_clipboard_get_err = env::var("SSH_CLIENT").is_err() && get_res.is_err();
    //   3. We might be able to get the result from clipboard but it could be empty
    let clipboard_not_populated = get_res.is_ok() && get_res.unwrap().is_empty();

    // Clipboard should be populated, but if any of the above edge cases are true,
    // then we need additional handling for possible errors or a final attempt
    // at setting the clipboard.
    if clipboard_unresponsive || local_clipboard_get_err || clipboard_not_populated {
        // If the clipboard is empty, then we failed to set the clipboard using
        // clipboard_anywhere; as such, let's try setting the clipboard using an
        // X11-aware clipboard manager
        let result = std::panic::catch_unwind(|| {
            let mut ctx = ClipboardContext::new().unwrap();
            ctx.set_contents(s.to_string())
                .expect("Failed to set contents of clipboard");
        });

        if result.is_err() {
            eprintln!("Warning: 2FA code could not be copied to clipboard");
        }
    }
}
