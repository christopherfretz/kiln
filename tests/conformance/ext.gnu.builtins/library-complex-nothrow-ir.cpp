// The <complex.h> functions are `nothrow` library builtins (`cproj` also
// `const`); a user declaration with their type gets `nounwind`, so the
// calls are `call`s under a pending cleanup (clang's
// `AddKnownFunctionAttributes`).
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
extern "C" double _Complex csqrt(double _Complex);
extern "C" float _Complex cexpf(float _Complex);
extern "C" double cabs(double _Complex);
extern "C" long double cargl(long double _Complex);
extern "C" double _Complex cproj(double _Complex);
extern "C" double _Complex cpow(double _Complex, double _Complex);

struct D { ~D(); };

double f(double _Complex z, float _Complex w, long double _Complex l) {
  D d;
  double _Complex a = csqrt(z);
  float _Complex b = cexpf(w);
  double _Complex c = cproj(z);
  double _Complex p = cpow(z, a);
  return cabs(a) + __real__ b + __real__ c + __real__ p + (double)cargl(l);
}
