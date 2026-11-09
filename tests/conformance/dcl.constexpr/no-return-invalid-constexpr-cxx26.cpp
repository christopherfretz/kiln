// [dcl.constexpr]/3 (C++26): with no return statement allowed (P2448R2), a
// constexpr function that cannot reach one never produces a constant
// expression, which -Winvalid-constexpr reports as an error, clang's verdict
// for each of these (WP-455, WP-1056; without the flag they are accepted,
// no-return-cxx23.cpp).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ args: -Winvalid-constexpr
//@ diag: 32:15 error "constexpr function never produces a constant expression"
//@ diag: 32:31 note "subexpression not valid in a constant expression"
//@ diag: 33:15 error "consteval function never produces a constant expression"
//@ diag: 33:34 note "subexpression not valid in a constant expression"
//@ diag: 34:15 error "constexpr function never produces a constant expression"
//@ diag: 34:24 note "subexpression not valid in a constant expression"
//@ diag: 35:15 error "constexpr function never produces a constant expression"
//@ diag: 35:33 note "non-constexpr function 'die' cannot be used in a constant expression"
//@ diag: 30:19 note "declared here"
//@ diag: 36:15 error "constexpr function never produces a constant expression"
//@ diag: 36:35 note "non-constexpr function 'unreachable' cannot be used in a constant expression"
//@ diag: 28:26 note "declared here"
//@ diag: 37:15 error "constexpr function never produces a constant expression"
//@ diag: 37:35 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 38:15 error "constexpr function never produces a constant expression"
//@ diag: 38:37 note "constexpr evaluation hit maximum step limit; possible infinite loop?"

namespace std {
[[noreturn]] inline void unreachable() { __builtin_unreachable(); }
}
[[noreturn]] void die();

constexpr int unreachable() { __builtin_unreachable(); }
consteval int ce_unreachable() { __builtin_unreachable(); }
constexpr int trap() { __builtin_trap(); }
constexpr int noreturn_call() { die(); }
constexpr int std_unreachable() { std::unreachable(); }
constexpr int forever() { for (;;); }
constexpr int spin() { while (true) {} }

