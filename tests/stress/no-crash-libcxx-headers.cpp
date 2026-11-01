// WP-141: with no error limit, kiln must diagnose (not crash on) whatever it
// cannot yet handle in these libc++ headers. Errors are fine; a panic, a
// signal, a hang, an internal-error exit code or a memory blow-up is not.
// The 214-test `decl has no Function payload` panic hid behind the default
// error limit of 20 in `<string>`.
//@ expect: no-crash
//@ args: -stdlib=libc++ -ferror-limit=0
//@ mem<=4096
#include <string>
#include <memory>
#include <chrono>
#include <optional>
#include <tuple>
