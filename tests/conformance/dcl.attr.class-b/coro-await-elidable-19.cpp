// Under clang 19 compatibility clang::coro_await_elidable and
// clang::coro_await_elidable_argument (clang 20) are unknown attributes,
// ignored with a warning (spec/landed/codegen/WP-813.md). WP-813.
//@ verify
//@ requires: clang<22
struct [[clang::coro_await_elidable]] T1 {}; // expected-warning {{unknown attribute 'coro_await_elidable' ignored}}
[[clang::coro_await_elidable]] int v1; // expected-warning {{unknown attribute 'coro_await_elidable' ignored}}
void f([[clang::coro_await_elidable_argument]] int p); // expected-warning {{unknown attribute 'coro_await_elidable_argument' ignored}}
[[clang::coro_await_elidable_argument]] int v2; // expected-warning {{unknown attribute 'coro_await_elidable_argument' ignored}}
struct [[clang::coro_await_elidable(1)]] T2 {}; // expected-warning {{unknown attribute 'coro_await_elidable' ignored}}

