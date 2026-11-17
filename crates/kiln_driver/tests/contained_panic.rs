//! The contained-panic scope of the crash hook (spec/driver.md "In-process
//! back end", WP-1559; `kiln_driver::crash::contained`). Its own test
//! binary: it installs the process's panic hook, and the first-panic
//! record is process-wide, so the scenarios run in one test, in order.
#![forbid(unsafe_code)]

use kiln_driver::crash::{contained, install_hook, recorded_message};

#[test]
fn contained_scopes_keep_the_first_panic_record() {
    install_hook();
    assert_eq!(recorded_message(), None);

    // A panic on the calling thread: an `Err` with its message, nothing
    // recorded.
    let r: Result<(), String> = contained(|| panic!("on the caller"));
    assert_eq!(r, Err("on the caller".to_owned()));
    assert_eq!(recorded_message(), None);

    // A panic on a thread started inside, re-raised by the caller as
    // `thread::scope` does: the worker's message.
    let r: Result<(), String> = contained(|| {
        std::thread::scope(|s| {
            s.spawn(|| panic!("on a worker"));
        });
    });
    assert_eq!(r, Err("on a worker".to_owned()));
    assert_eq!(recorded_message(), None);

    // A worker's panic the caller handles itself (it joins and answers):
    // `Ok`, still nothing recorded.
    let r = contained(|| std::thread::spawn(|| panic!("handled")).join().is_err());
    assert_eq!(r, Ok(true));
    assert_eq!(recorded_message(), None);

    // Nested scopes: the inner one takes its panic, the outer sees none.
    let r = contained(|| contained(|| -> u8 { panic!("inner") }));
    assert_eq!(r, Ok(Err("inner".to_owned())));
    assert_eq!(recorded_message(), None);

    // A real panic after the scopes is the process's first.
    let after = std::panic::catch_unwind(|| panic!("a real crash"));
    assert!(after.is_err());
    assert_eq!(recorded_message().as_deref(), Some("a real crash"));

    // A panic inside a later scope leaves that record alone.
    let r: Result<(), String> = contained(|| panic!("contained later"));
    assert_eq!(r, Err("contained later".to_owned()));
    assert_eq!(recorded_message().as_deref(), Some("a real crash"));
}
