// [dcl.attr.grammar]/4 (GNU extension): `visibility`, `type_visibility` and
// `pcs` take a leading identifier as clang does, then reject it after the
// subject check with "attribute requires a string"; `weakref` and the
// other string attributes reject an identifier at parse (spec/parse.md
// §6.2 "Argument table", WP-1881). Verified against clang 19 and 22.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:27 error "'visibility' attribute requires a string"
//@ diag: 33:19 error "'visibility' attribute requires a string"
//@ diag: 34:35 error "expected string literal as argument of 'visibility' attribute"
//@ diag: 35:39 error "'type_visibility' attribute requires a string"
//@ diag: 36:33 error "'type_visibility' attribute requires a string"
//@ diag: 37:20 error "'pcs' attribute requires a string"
//@ diag: 37:16 warning "'pcs' calling convention is not supported for this target"
//@ diag: 38:12 error "'pcs' attribute requires a string"
//@ diag: 38:3 warning "'pcs' calling convention is not supported for this target"
//@ diag: 39:27 error "expected string literal as argument of 'visibility' attribute"
//@ diag: 40:20 error "expected string literal as argument of 'pcs' attribute"
//@ diag: 42:33 error "expected ')'"
//@ diag: 42:33 error "expected ')'"
//@ diag: 42:39 error "expected unqualified-id"
//@ diag: 43:31 error "'__visibility__' attribute requires a string"
//@ diag: 44:23 error "'__visibility__' attribute requires a string"
//@ diag: 45:24 error "'__pcs__' attribute requires a string"
//@ diag: 45:16 warning "'__pcs__' calling convention is not supported for this target"
//@ diag: 46:16 error "'pcs' attribute requires a string"
//@ diag: 46:3 warning "'pcs' calling convention is not supported for this target"
//@ diag: 47:24 error "expected string literal as argument of 'weakref' attribute"
//@ diag: 48:16 warning "'pcs' calling convention is not supported for this target"

__attribute__((visibility(hidden))) void f1();
[[gnu::visibility(hidden)]] void f2();
__attribute__((visibility(hidden, 1))) void f3();
struct __attribute__((type_visibility(hidden))) S4 {};
struct [[clang::type_visibility(hidden)]] S5 {};
__attribute__((pcs(aapcs))) void f4();
[[gnu::pcs(aapcs)]] void f5();
__attribute__((visibility(1))) void f6();
__attribute__((pcs(1))) void f7();
__attribute__((visibility("hidden"))) void f8();
__attribute__((visibility(hidden + 1))) void f9();
__attribute__((__visibility__(hidden))) void f11();
[[gnu::__visibility__(hidden)]] void f12();
__attribute__((__pcs__(aapcs))) void f14();
[[__gnu__::pcs(aapcs)]] void f15();
__attribute__((weakref(hidden, 1))) static void f16();
__attribute__((pcs("aapcs"))) void f17();
