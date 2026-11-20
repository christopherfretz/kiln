// [dcl.fct]/22, [expr.prim.lambda.general]: a lambda-expression in the
// trailing requires-clause of an abbreviated function template does not
// end the declarator's invented template head: the function is still a
// template, and a call of it resolves to a specialization (found running
// GCC's C++ testsuite, where the call stayed unresolved and failed in
// code generation).
//@ expect: run
//@ stdout: 1 3
extern "C" int printf(const char*, ...);

int one(auto p) requires requires { [](int q) { return q; }(p); } { return p; }
int four(auto... a) requires requires { []<int i = 0> { return i; }(); } {
  return sizeof...(a);
}

int main() {
  printf("%d %d\n", one(1), four(1, 2, 3));
}
