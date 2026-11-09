// [dcl.constexpr]/3 (C++11): the body of a constexpr function other than a
// constructor contains exactly one return statement, and no iteration
// statement. clang `CheckConstexprMissingReturn` checks only that a return
// statement exists: "no return statement in constexpr function", for any
// return type (dependent or not) before C++14, whatever ends the function
// instead (`__builtin_unreachable`, `__builtin_trap`, a [[noreturn]] call,
// `std::unreachable`), and the never-constant check does not run (WP-455,
// WP-1056). The other C++11 statements are C++14 extension warnings, off
// here.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ args: -Wno-c++14-extensions
//@ diag: 33:15 error "no return statement in constexpr function"
//@ diag: 34:15 error "no return statement in constexpr function"
//@ diag: 35:15 error "no return statement in constexpr function"
//@ diag: 36:15 error "no return statement in constexpr function"
//@ diag: 37:15 error "no return statement in constexpr function"
//@ diag: 38:15 error "no return statement in constexpr function"
//@ diag: 39:32 error "no return statement in constexpr function"
//@ diag: 40:34 error "no return statement in constexpr function"
//@ diag: 43:13 error "no return statement in constexpr function"
//@ diag: 47:27 error "statement not allowed in constexpr function"
//@ diag: 48:24 error "statement not allowed in constexpr function"
//@ diag: 49:29 error "statement not allowed in constexpr function"

namespace std {
[[noreturn]] inline void unreachable() { __builtin_unreachable(); }
}
[[noreturn]] void die();

// No return statement: an error for any return type before C++14.
constexpr int empty() {}
constexpr int unreachable() { __builtin_unreachable(); }
constexpr int trap() { __builtin_trap(); }
constexpr int noreturn_call() { die(); }
constexpr int std_unreachable() { std::unreachable(); }
constexpr int throws(int x) { if (x) throw 1; }
template <class T> constexpr T dependent() {}
template <class T> constexpr int non_dependent() {}
struct S {
  constexpr S() {}
  constexpr operator int() const {}
};

// An iteration statement is not allowed in C++11 (nor is `continue`).
constexpr int forever() { for (;;); }
constexpr int spin() { while (true) {} }
constexpr int once(int x) { do { return x; } while (false); }

// A return statement anywhere in the body satisfies the rule.
constexpr int branch(bool b) { if (b) { return 1; } else { throw 2; } }
constexpr int sw(int x) { switch (x) { case 0: return 1; default: throw 0; } }
