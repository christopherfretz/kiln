// [dcl.attr.grammar]/5 with clang's `coro_only_destroy_when_complete`
// ([dcl.fct.def.coroutine] extension): it appertains to classes only
// (ignored with a warning elsewhere) and takes no arguments.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang<22
// clang 22 prints attribute names scoped (`'clang::coro_only_destroy_when_complete'`); WP-377 owns the compat-22 wording
//@ diag: 13:3 warning "'coro_only_destroy_when_complete' attribute only applies to classes"
//@ diag: 14:3 warning "'coro_only_destroy_when_complete' attribute only applies to classes"
//@ diag: 15:10 error "'coro_only_destroy_when_complete' attribute takes no arguments"
struct [[clang::coro_only_destroy_when_complete]] S {};
[[clang::coro_only_destroy_when_complete]] int x;
[[clang::coro_only_destroy_when_complete]] void f();
struct [[clang::coro_only_destroy_when_complete(1)]] T {};
union __attribute__((coro_only_destroy_when_complete)) U {};
