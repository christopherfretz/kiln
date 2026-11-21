// gnu-attribute-arguments-parser-names.cpp as clang 22 parses it
// (WP-1853): its string literal positions count the identifier argument,
// so `nodiscard(x, 1)` asks for none; `riscv_vls_cc` (new in clang 22) is
// another target's attribute, its argument an expression. The clang 19
// sibling is gnu-attribute-arguments-parser-names-clang19.cpp.
//@ verify
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu
__attribute__((nodiscard(undeclared, 1))) int g1(); // expected-warning {{unknown attribute 'nodiscard' ignored}}
__attribute__((riscv_vls_cc(undeclared))) void h1(); // expected-error {{use of undeclared identifier 'undeclared'}}
