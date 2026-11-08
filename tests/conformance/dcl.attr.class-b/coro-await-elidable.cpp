// clang::coro_await_elidable (classes) and
// clang::coro_await_elidable_argument (parameters), clang 20's: their
// subjects and arguments, and __has_cpp_attribute reports them; clang 19
// does not know them (spec/landed/codegen/WP-813.md). [dcl.attr.grammar]/5.
// WP-813.
//@ verify
//@ requires: clang>=22
struct [[clang::coro_await_elidable]] T1 {};
[[clang::coro_await_elidable]] int v1; // expected-warning {{'clang::coro_await_elidable' attribute only applies to classes}}
void f([[clang::coro_await_elidable_argument]] int p);
[[clang::coro_await_elidable_argument]] int v2; // expected-warning {{'clang::coro_await_elidable_argument' attribute only applies to parameters}}
struct [[clang::coro_await_elidable(1)]] T2 {}; // expected-error {{'clang::coro_await_elidable' attribute takes no arguments}}
#if !__has_cpp_attribute(clang::coro_await_elidable) || !__has_cpp_attribute(clang::coro_await_elidable_argument)
#error "not reported"
#endif
