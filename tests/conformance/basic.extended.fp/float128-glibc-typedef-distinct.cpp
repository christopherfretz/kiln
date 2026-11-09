// [conv.fctptr], [basic.extended.fp] (WP-1058): without glibc's x86-64
// `typedef __float128 _Float128;` (spec/types.md "Extended floating-point
// types"), `_Float128` and `__float128` are distinct types (decision (B)
// of WP-1000), so a `_Float128` function does not convert to a pointer to
// a `__float128` one, as in GCC 14. The same typedef in user code (not a
// system header) is an error and does not enable the conversion.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ std: c++23
//@ strict-diags
//@ diag: 16:14 error "cannot initialize a variable of type '__float128 (*)(__float128)' with an lvalue of type '_Float128 (_Float128)': type mismatch at 1st parameter ('__float128' vs '_Float128')"
//@ diag: 17:20 error "cannot combine with previous '__float128' declaration specifier"
//@ diag: 18:14 error "cannot initialize a variable of type '__float128 (*)(__float128)' with an lvalue of type '_Float128 (_Float128)': type mismatch at 1st parameter ('__float128' vs '_Float128')"
//@ skip(clang): clang 19 and 22 have no _Float128 (GCC is the reference)
extern "C" _Float128 logf128(_Float128);
__float128 (*p)(__float128) = logf128;
typedef __float128 _Float128;
__float128 (*q)(__float128) = logf128;
_Float128 (*ok)(_Float128) = logf128;
