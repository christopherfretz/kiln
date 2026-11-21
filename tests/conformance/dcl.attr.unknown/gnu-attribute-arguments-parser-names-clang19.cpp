// gnu-attribute-arguments-parser-names.cpp as clang 19 parses it
// (WP-1853): its string literal positions start after the identifier
// argument, so `nodiscard(x, 1)` wants one at `1`; `riscv_vls_cc` and
// `coro_await_elidable` are not clang 19 attributes, `nvptx_kernel` is
// another target's. The clang 22 sibling is
// gnu-attribute-arguments-parser-names-clang22.cpp.
//@ verify
//@ requires: clang<22
//@ args: --target=aarch64-unknown-linux-gnu
__attribute__((nodiscard(undeclared, 1))) int g1(); // expected-error {{expected string literal as argument of 'nodiscard' attribute}}
__attribute__((riscv_vls_cc(undeclared))) void h1(); // expected-warning {{unknown attribute 'riscv_vls_cc' ignored}}
__attribute__((coro_await_elidable(undeclared))) void h2(); // expected-warning {{unknown attribute 'coro_await_elidable' ignored}}
__attribute__((nvptx_kernel(undeclared))) void h3(); // expected-error {{use of undeclared identifier 'undeclared'}}
