// [dcl.constexpr]/3 (C++20, P1668): an asm-declaration may appear in a
// constexpr function, but evaluating one is not a core constant expression
// ([expr.const]/10); an asm statement on a path not taken is fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:5 note "subexpression not valid in a constant expression"
//@ diag: 15:15 note "in call to 'f(1)'"
constexpr int f(int x) {
  if (x > 0)
    asm volatile("" : "+r"(x));
  return x;
}
static_assert(f(0) == 0);
static_assert(f(1) == 1);
constexpr int g(int x) {
  if consteval { return x; } else { asm("" :: "r"(x)); return x; }
}
static_assert(g(2) == 2);
int h(int x) { return f(x) + g(x); }
