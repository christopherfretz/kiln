// A function pointer conversion may drop GNU `noreturn` from a function
// type (clang `IsFunctionConversion`). Adding it is not a standard
// conversion; clang 19/22 reject it, GCC 16.1 accepts it, and kiln
// accepts it with a warning (WP-1551, conv.fctptr/gnu-noreturn-add-run.cpp).
// The standard `[[noreturn]]` appertains to the function, not its type,
// so `std_noreturn` has the plain type `void ()`.
//@ oracle: gcc
//@ skip(clang): clang 19/22 reject a conversion that adds noreturn to a function pointer
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:13 warning "conversion from 'void (*)()' to 'fail_fn' (aka 'void (*)() __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
//@ diag: 16:13 warning "conversion from 'void (*)()' to 'fail_fn' (aka 'void (*)() __attribute__((noreturn))') adds 'noreturn' to the function type (a GCC extension)"
typedef void (*fail_fn)() __attribute__((noreturn));
[[noreturn]] void std_noreturn();
fail_fn a = std_noreturn;
fail_fn b = &std_noreturn;
