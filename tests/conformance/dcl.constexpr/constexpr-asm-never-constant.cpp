// [dcl.constexpr]/6 (C++20, an IFNDR rule clang diagnoses as an error under
// -Winvalid-constexpr): a constexpr function whose every evaluation reaches
// an asm-declaration never produces a constant expression, with or without
// parameters (WP-891 probe, WP-915): clang's evaluator has no case for one.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "constexpr function never produces a constant expression"
//@ diag: 12:3 note "subexpression not valid in a constant expression"

constexpr int g(int x) {
  asm("");
  return x;
}
