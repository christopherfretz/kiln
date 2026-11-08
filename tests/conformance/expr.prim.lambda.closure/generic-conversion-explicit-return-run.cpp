// [expr.prim.lambda.closure]/10: a generic lambda's conversion to a
// function pointer, deduced from the target type, returns the address of
// an invoker specialization that calls the call operator's
// specialization for the same arguments, so that specialization is
// odr-used and instantiated (clang
// `DefineImplicitLambdaToFunctionPointerConversion`). With an explicit
// return type nothing else instantiates it (a deduced return type would);
// kiln marked only the call operator template, leaving the specialization
// undefined: a link error. WP-799.
//@ expect: run
//@ strict-diags
//@ stdout: 5|8|2.5
extern "C" int printf(const char *, ...);

struct M {
  int v;
};

struct Tactic {
  using Factory = int *(*)(M &, const int &, double &);
  Factory f;
  explicit Tactic(Factory f) : f(f) {}
};

inline Tactic *make() {
  return new Tactic([](auto &m, auto &p, auto &) -> int * {
    m.v += p;
    return &m.v;
  });
}

using Unary = double (*)(double);

int main() {
  M m{4};
  int p = 1;
  double s = 0;
  Tactic *t = make();
  printf("%d|", *t->f(m, p, s));
  int (*g)(int &) = [](auto &x) -> int { return x * 2; };
  int x = 4;
  printf("%d|", g(x));
  Unary h = [](auto d) -> double { return d / 2; };
  printf("%g\n", h(5.0));
  delete t;
}
