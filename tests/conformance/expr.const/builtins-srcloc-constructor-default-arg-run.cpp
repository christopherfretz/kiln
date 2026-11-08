// [support.srcloc.cons]/2, [dcl.fct.default]/9: source_location::current()
// as a constructor's default argument takes the construction's location
// and function, for every form of construction (`T x;`, `T x(a)`,
// `T x{a}`, `T{a}`, `new T`, a base or member subobject, a template),
// not the default argument's own line. WP-811 (Bitcoin Core's
// `SourceLocation{__func__}` log rate-limiter keys).
//@ expect: run
//@ strict-diags
//@ stdout: 29 30 31 32 33 34 25 36 27 | 5 5 6 11 10 14 16 7 38
//@ stdout: D::D()|T make() [T = E]|int main() | 23 24
#include <source_location>
extern "C" int printf(const char *, ...);
struct S {
  unsigned n, c; const char* f;
  constexpr S(std::source_location l = std::source_location::current())
      : n(l.line()), c(l.column()), f(l.function_name()) {}
};
struct E {
  constexpr explicit E(int, std::source_location l = std::source_location::current())
      : n(l.line()), c(l.column()), f(l.function_name()) {}
  unsigned n, c; const char* f;
};
constexpr E k(1);
constexpr unsigned g() { E x{2}; return x.n; }
struct D : S { D() {} };
struct M { S s; };
template <class T> T make() { return T(1); }
int main() {
  S s;
  E e(1);
   E e2{1};
  auto e3{E{1}};
  E e4 = E(1);
  S* p = new S;
  D d;
  M m{};
  E t = make<E>();
  static_assert(k.n == 23 && g() == 24);
  unsigned ns[] = {s.n, e.n, e2.n, e3.n, e4.n, p->n, d.n, m.s.n, t.n};
  unsigned cs[] = {s.c, e.c, e2.c, e3.c, e4.c, p->c, d.c, m.s.c, t.c};
  for (unsigned x : ns) printf("%u ", x);
  printf("|");
  for (unsigned x : cs) printf(" %u", x);
  printf("\n%s|%s|%s | %u %u\n", d.f, t.f, m.s.f, k.n, g());
  delete p;
}
