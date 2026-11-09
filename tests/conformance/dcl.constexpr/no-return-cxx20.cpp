// [dcl.constexpr]/3 (C++20, clang `CheckConstexprMissingReturn`): a
// constexpr or consteval function, or the call operator of a lambda
// declared so, other than a constructor or destructor, whose body has no
// return statement and whose return type is neither void nor dependent is
// an error ("no return statement in constexpr|consteval function"). A
// return statement inside a statement expression does not count; one in a
// discarded `if constexpr` branch does. The function is then invalid, and the
// never-constant check skips it (WP-455, WP-1056).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 26:15 error "no return statement in constexpr function"
//@ diag: 27:15 error "no return statement in consteval function"
//@ diag: 28:15 error "no return statement in consteval function"
//@ diag: 29:15 error "no return statement in constexpr function"
//@ diag: 30:15 error "no return statement in constexpr function"
//@ diag: 34:11 error "no return statement in constexpr function"
//@ diag: 35:11 error "no return statement in consteval function"
//@ diag: 36:11 error "no return statement in consteval function"
//@ diag: 37:11 error "no return statement in constexpr function"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:60 note "control reached end of constexpr function"

[[noreturn]] void die();

constexpr int unreachable() { __builtin_unreachable(); }
consteval int ce_unreachable() { __builtin_unreachable(); }
consteval int ce_trap() { __builtin_trap(); }
constexpr int noreturn_call() { die(); }
constexpr int in_stmt_expr() { ({ return 1; }); }

// A lambda declared constexpr or consteval is checked; the diagnostic is at
// its introducer.
auto l1 = []() constexpr -> int {};
auto l2 = []() consteval -> int {};
auto l3 = [](auto) consteval -> int {};
auto l4 = []<class T>() constexpr -> int {};
auto l5 = []() -> int { __builtin_unreachable(); };
auto l6 = [](auto) constexpr {};

// Not for a dependent return type, nor for an instantiation, whose missing
// return is then only -Wreturn-type.
template <class T> constexpr T dependent() {}
int a = dependent<int>();

// A destructor and a constructor have no return statement.
struct D {
  constexpr D() {}
  constexpr ~D() {}
};

// A discarded return statement still counts; the function then never
// produces a constant expression.
constexpr int discarded() { if constexpr (false) return 1; }
