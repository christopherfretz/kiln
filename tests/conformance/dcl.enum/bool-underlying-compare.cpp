// [dcl.enum]/5, [basic.fundamental]/10: an enumeration whose underlying
// type is `bool` has the values of `bool`; clang represents it as `bool`
// (an `i8` object, an `i1` value: `hasBooleanRepresentation`), so a loaded
// member compares, switches and orders as an `i1` against its `i1`
// enumerators. The shape is a downstream Handler (a
// `const enum class Policy : bool` member tested with `==` in
// out-of-line member functions).
//@ expect: run
//@ stdout: 3 1 3 1
//@ stdout: yes no
//@ stdout: 0 1 0 1 1 0
//@ stdout: 2 2 1 7
extern "C" int printf(const char *, ...);

enum class P : bool { NO, YES };
enum U : bool { UN, UY };

struct S {
  const P p;
  int run() const;
};
int S::run() const {
  if (p == P::YES) return 3;
  return 1;
}

struct T {
  U u;
  int run() const { return u == UY ? 3 : 1; }
};

const char *name(P p) {
  switch (p) {
  case P::NO: return "no";
  case P::YES: return "yes";
  }
  return "?";
}

int uswitch(U u) {
  switch (u) {
  case UN: return 1;
  case UY: return 2;
  }
  return 0;
}

int main() {
  S a{P::YES}, b{P::NO};
  T c{UY}, d{UN};
  printf("%d %d %d %d\n", a.run(), b.run(), c.run(), d.run());
  printf("%s %s\n", name(a.p), name(b.p));
  P y = P::YES, n = P::NO;
  printf("%d %d %d %d %d %d\n", y < n, n < y, y != P::YES, n != y, y >= n,
         c.u < d.u);
  int s = 0;
  switch (a.p) {
  case P::YES: s = 2; break;
  default: s = 9;
  }
  printf("%d %d %d %d\n", s, uswitch(c.u), uswitch(d.u), c.u + UY + 5);
}
