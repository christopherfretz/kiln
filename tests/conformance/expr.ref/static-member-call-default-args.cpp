// [expr.ref], [dcl.fct.default]/1, [expr.const]/16: a static member
// function named by a class member access (`o.f(...)`, `p->f(...)`,
// `this->f(...)`) is called as when named by its name: the default
// arguments of its omitted trailing parameters are used, and a consteval
// one is an immediate invocation, also with a default argument. Run
// through codegen.
//@ expect: run
//@ strict-diags
//@ stdout: 8 8 8 8 5 9
extern "C" int printf(const char*, ...);

struct S {
  static int f(int a, int b = 7) { return a + b; }
  static consteval int c(int a = 5) { return a; }
  int self() { return this->f(1); }
};

int main() {
  S s;
  S* p = &s;
  printf("%d %d %d %d %d %d\n", s.f(1), p->f(1), S::f(1), s.self(), s.c(), S{}.c(9));
  return 0;
}
