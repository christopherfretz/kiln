// [expr.prim.lambda.closure]/9-10, [expr.const]/22 (P2564, WP-158 item d):
// escalation is tracked per specialization of a generic lambda's
// conversion function template: a specialization whose call operator
// specialization escalated is immediate (its result cannot leave an
// immediate function); other specializations are not.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang (19 and 22) does not make the conversion of an escalated generic lambda immediate and accepts every use (cf. consteval-conversion.cpp)
//@ diag: 19:18 error "call to immediate function '(lambda)::operator auto (*)(int)<int>' is not a constant expression"
//@ diag: 19:18 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 18:12 note "declared here"
//@ diag: 22:19 error "call to immediate function '(lambda)::operator auto (*)(int)<int>' is not a constant expression"
//@ diag: 22:19 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 18:12 note "declared here"
//@ requires: clang>=22
// kiln follows clang 22 naming the closure `(lambda)`
consteval int id(int x) { return x; }
auto esc = [](auto x) { if constexpr (sizeof(x) == 4) return id(x); else return x; };
int (*p1)(int) = esc;
long (*p2)(long) = esc;
void use() {
  int (*q)(int) = esc;
  long (*w)(long) = esc;
}
consteval int in_immediate() {
  int (*a)(int) = esc;
  return a(1);
}
static_assert(in_immediate() == 1);
