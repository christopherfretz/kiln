// [intro.execution]/5 and -Waddress-of-packed-member: an explicit cast
// ([expr.cast], [expr.static.cast], [expr.reinterpret.cast],
// [expr.type.conv]) of `&m` drops the misaligned-member candidate when the
// cast is built, as clang `BuildCStyleCastExpr` / `BuildCXXNamedCast` do,
// so a full-expression nested later in the same enclosing expression (a
// template argument, a lambda body, a function body instantiated
// mid-expression) does not report it (WP-659; spec/sema.md §5.14). An
// implicit conversion drops it only when the call converts its arguments,
// after the nested full-expression has already reported it (clang does
// the same: lines 29-30).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 no longer reports lines 29-30 (the implicit-conversion case); kiln matches clang 19 here (verified with --compiler clang++-19); clang-22 parity is WP-892
//@ diag: 25:23 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 29:17 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 30:17 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 32:23 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"

struct __attribute__((packed)) P { char c; int i; };
P p;
template <class T> constexpr int ft() { return T(4); }
template <int N> struct A { static constexpr int w = N; };
template <class T> struct X { static constexpr int v = sizeof(T); };
template <class T> auto af(T t) { return t + 1; }
template <class T> struct C {
  int g() { int *q = &p.i; return *q; }
};
void f(char *, int);
void fv(void *, int);
void h1() { fv(&p.i, [] { return 1; }()); }
void h2() { fv(&p.i, A<3>::w); }
void h3() { fv(&p.i, 0); }
void h4() { int *q = &p.i; (void)q; }

void n1() { f((char *)&p.i, A<ft<short>()>::w); }
void n2() { f((char *)&p.i, A<X<int>::v>::w); }
void n3() { f((char *)&p.i, A<3>::w); }
void n4() { f((char *)&p.i, sizeof(int[ft<long>()])); }
void n5() { f((char *)&p.i, af(1)); }
void n6() { f((char *)&p.i, C<int>().g()); }
void n7() { f((char *)&p.i, [] { return 1; }()); }
void n8() { f(reinterpret_cast<char *>(&p.i), [] { return 1; }()); }
void n9() { fv(static_cast<void *>(&p.i), A<ft<int>()>::w); }
void n10() { f((char *)&p.i, (fv((void *)&p.i, 0), 1)); }
