// [dcl.constexpr]/6 (C++20, an IFNDR rule clang diagnoses as an error under
// -Winvalid-constexpr): a constexpr function whose every evaluation reaches
// an asm-declaration never produces a constant expression. kiln's
// never-constant check only tries parameterless functions (WP-891 probe).
//@ xfail(kiln): WP-891 follow-up, the never-constant check skips functions with parameters
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "constexpr function never produces a constant expression"
//@ diag: 13:3 note "subexpression not valid in a constant expression"

constexpr int g(int x) {
  asm("");
  return x;
}
