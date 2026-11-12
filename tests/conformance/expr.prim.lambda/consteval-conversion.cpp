// [expr.prim.lambda.closure]/10, [expr.const]/22 (P2564, WP-137): the
// conversion to function pointer of a consteval or escalated lambda is an
// immediate function, so it may be used inside an immediate function but
// its result (a pointer to the immediate static invoker) cannot leave one.
// C++23: with C++26 reflection kiln follows P4101R1, where such a pointer
// is a consteval-only value (expr.const.const/p3-immediate-function-pointer-*).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 28:18 error "call to consteval function '(lambda)::operator int (*)(int)' is not a constant expression"
//@ diag: 28:18 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 26:11 note "declared here"
//@ diag: 29:18 error "call to immediate function '(lambda)::operator int (*)(int)' is not a constant expression"
//@ diag: 29:18 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 27:12 note "declared here"
//@ diag: 31:20 error "call to consteval function '(lambda)::operator int (*)(int)' is not a constant expression"
//@ diag: 31:20 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 26:11 note "declared here"
//@ diag: 32:19 error "call to immediate function '(lambda)::operator int (*)(int)' is not a constant expression"
//@ diag: 32:19 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 27:12 note "declared here"
//@ xfail(clang): clang (19 and 22) marks only the static invoker immediate and rejects every use of the conversion ("cannot take address of consteval call operator ... outside of an immediate invocation" at the lambda)
//@ requires: clang>=22
// kiln follows clang 22 naming the closure `(lambda)`
consteval int id(int x) { return x; }
auto ce = [](int x) consteval { return x + 1; };
auto esc = [](int x) { return id(x); };
int (*p1)(int) = ce;
int (*p2)(int) = esc;
void use() {
  int (*q)(int) = +ce;
  int (*w)(int) = esc;
}
consteval int in_immediate() {
  int (*a)(int) = ce;
  int (*b)(int) = esc;
  return a(1) + b(2);
}
static_assert(in_immediate() == 4);
