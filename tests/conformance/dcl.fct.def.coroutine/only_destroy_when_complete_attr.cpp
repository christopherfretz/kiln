// [dcl.attr.grammar]/5 with clang's `coro_only_destroy_when_complete`
// ([dcl.fct.def.coroutine] extension): it appertains to classes only
// (ignored with a warning elsewhere) and takes no arguments.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 11:3 warning "'coro_only_destroy_when_complete' attribute only applies to classes"
//@ diag: 12:3 warning "'coro_only_destroy_when_complete' attribute only applies to classes"
//@ diag: 13:10 error "'coro_only_destroy_when_complete' attribute takes no arguments"
struct [[clang::coro_only_destroy_when_complete]] S {};
[[clang::coro_only_destroy_when_complete]] int x;
[[clang::coro_only_destroy_when_complete]] void f();
struct [[clang::coro_only_destroy_when_complete(1)]] T {};
union __attribute__((coro_only_destroy_when_complete)) U {};
