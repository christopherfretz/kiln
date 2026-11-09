// [dcl.constexpr]/3 (C++14 to C++20; the C++11 rule as clang keeps it,
// `CheckConstexprMissingReturn`): a constexpr function other than a
// constructor whose body has no return statement is an error, "no return
// statement in constexpr function", unless its return type is void or
// dependent (a templated function's undeduced `auto` included). Whatever
// ends the function instead (`__builtin_unreachable`, `__builtin_trap`, a
// [[noreturn]] call, `std::unreachable`, an infinite loop) does not count,
// nor does a return statement in a lambda; the never-constant check and
// -Wreturn-type are skipped (WP-455, WP-1056).
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ diag: 36:15 error "no return statement in constexpr function"
//@ diag: 37:15 error "no return statement in constexpr function"
//@ diag: 38:15 error "no return statement in constexpr function"
//@ diag: 39:15 error "no return statement in constexpr function"
//@ diag: 40:15 error "no return statement in constexpr function"
//@ diag: 41:15 error "no return statement in constexpr function"
//@ diag: 42:15 error "no return statement in constexpr function"
//@ diag: 43:15 error "no return statement in constexpr function"
//@ diag: 44:27 warning "function try block in constexpr function is a C++20 extension"
//@ diag: 44:15 error "no return statement in constexpr function"
//@ diag: 47:15 error "no return statement in constexpr function"
//@ diag: 53:13 error "no return statement in constexpr function"
//@ diag: 54:17 error "no return statement in constexpr function"
//@ diag: 56:34 error "no return statement in constexpr function"
//@ diag: 58:17 error "no return statement in constexpr function"
//@ diag: 74:28 warning "use of this statement in a constexpr function is a C++23 extension"

namespace std {
[[noreturn]] inline void unreachable() { __builtin_unreachable(); }
}
[[noreturn]] void die();

// Ending the function some other way is no return statement (WP-1056).
constexpr int unreachable() { __builtin_unreachable(); }
constexpr int trap() { __builtin_trap(); }
constexpr int noreturn_call() { die(); }
constexpr int std_unreachable() { std::unreachable(); }
constexpr int forever() { for (;;); }
constexpr int spin() { while (true) {} }
constexpr int empty() {}
constexpr int throws(int x) { if (x) throw 1; }
constexpr int try_block() try {} catch (...) {}

// A return statement in a lambda does not count.
constexpr int in_lambda() { [] { return 1; }(); }

// Members, a conversion function, and a non-dependent return type in a
// template, checked at the definition.
struct S {
  constexpr S() {}
  constexpr operator int() const {}
  constexpr int member() const {}
};
template <class T> constexpr int non_dependent() {}
template <class T> struct A {
  constexpr int f() const {}
};

// A void or dependent return type is fine from C++14 on.
constexpr void nothing() {}
constexpr void nothing2(int x) { if (x) throw x; }
template <class T> constexpr T dependent() {}
template <class T> constexpr auto deduced() {}
template <class T> struct B {
  constexpr T g() const {}
  constexpr auto h() const {}
};
constexpr auto deduced_void() {}

// A return statement anywhere in the body satisfies the rule.
constexpr int nested(int x) { while (x) { return 1; } throw 2; }
constexpr int labelled() { l: return 1; }
