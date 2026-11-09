// [dcl.constexpr]/3 (C++23, P2448R2): a constexpr function need not contain
// a return statement, whatever its return type: clang's missing-return check
// stops before C++23, so these are accepted (with -Wreturn-type for the one
// that falls off its end). Their C++14 forms are errors (WP-455, WP-1056).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ diag: 15:24 warning "non-void function does not return a value"

namespace std {
[[noreturn]] inline void unreachable() { __builtin_unreachable(); }
}
[[noreturn]] void die();

constexpr int empty() {}
constexpr int unreachable() { __builtin_unreachable(); }
consteval int ce_unreachable() { __builtin_unreachable(); }
constexpr int trap() { __builtin_trap(); }
constexpr int noreturn_call() { die(); }
constexpr int std_unreachable() { std::unreachable(); }
constexpr int forever() { for (;;); }
constexpr int spin() { while (true) {} }
auto l = []() constexpr -> int { __builtin_unreachable(); };
struct S {
  constexpr operator int() const { __builtin_unreachable(); }
};

int main() {}
