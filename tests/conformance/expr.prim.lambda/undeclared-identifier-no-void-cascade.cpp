// [expr.prim.lambda.closure]/7, [dcl.spec.auto.general]/12: an undeclared
// identifier in the body of a lambda outside any template is one error.
// The call operator's placeholder return type is not deduced `void` from
// the failed `return` (clang 19: the operand `containsErrors()` and the
// operator is invalid), so neither a call nor a use of its result
// cascades. A generic lambda's specialization is invalid in the same way:
// the first call is silent, a later one is a substitution failure.
// clang 22 deduces `void` here and reports each use; kiln keeps clang
// 19's single error (spec/sema-templates.md §5.5; WP-434).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:23 error "use of undeclared identifier 'nope1'"
//@ diag: 26:21 error "use of undeclared identifier 'nope2'"
//@ diag: 27:22 error "use of undeclared identifier 'nope3'"
//@ diag: 31:17 error "use of undeclared identifier 'nope4'"
//@ diag: 32:36 error "use of undeclared identifier 'nope5'"
//@ diag: 34:12 error "no matching function for call to object of type '(lambda at "
//@ diag: 32:12 note "candidate template ignored: substitution failure [with x:auto = int]"
//@ diag: 36:32 error "use of undeclared identifier 'nope6'"
//@ diag: 38:24 error "use of undeclared identifier 'nope7'"
//@ diag: 43:24 error "use of undeclared identifier 'nope8'"
struct A {
  int v = [] { return nope1; }();
};
int k = [] { return nope2; }();
auto p = [] { return nope3; };
int q = p();
void f(int);
void use() {
  f([] { return nope4; }());
  auto r = [](auto x) { return x + nope5; };
  int s = r(1);
  int s2 = r(2);
}
int ret() { return [] { return nope6; }(); }
auto ret2() {
  auto l = [] { return nope7; };
  return l();
}
int ret3() { return ret2(); }
void g() {
  auto x = [] { return nope8; };
  decltype(x()) y = 1;
}
