// A function pointer conversion may drop GNU `noreturn` from a function
// type but never add it (clang `IsFunctionConversion`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:9 error "cannot initialize a variable of type 'fail_fn' (aka 'void (*)() __attribute__((noreturn))') with an lvalue of type 'void ()'"
//@ diag: 10:9 error "cannot initialize a variable of type 'fail_fn' (aka 'void (*)() __attribute__((noreturn))') with an rvalue of type 'void (*)()'"
typedef void (*fail_fn)() __attribute__((noreturn));
[[noreturn]] void std_noreturn();
fail_fn a = std_noreturn;
fail_fn b = &std_noreturn;
