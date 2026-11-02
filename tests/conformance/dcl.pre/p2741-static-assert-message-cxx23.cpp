// [dcl.pre]/11 (P2741R3): before C++26 a static_assert-message that is
// not a string literal is accepted as an extension (clang 22,
// -Wc++26-extensions); a string literal is not affected.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 18:22 warning "'static_assert' with a user-generated message is a C++26 extension [-Wc++26-extensions]"
//@ diag: 18:15 error "static assertion failed: old"
//@ diag: 19:15 error "static assertion failed: literal"
//@ diag: 20:21 warning "'static_assert' with a user-generated message is a C++26 extension [-Wc++26-extensions]"
//@ diag: 21:61 warning "'static_assert' with a user-generated message is a C++26 extension [-Wc++26-extensions]"
struct Msg {
  const char* p;
  constexpr const char* data() const { return p; }
  constexpr unsigned long size() const { return 3; }
};
static_assert(false, Msg{"old"});
static_assert(false, "literal");
static_assert(true, Msg{"yes"});
template <class T> void f() { static_assert(sizeof(T) == 1, T{"tpl"}); }
