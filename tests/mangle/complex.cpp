// GNU _Complex mangles as `C` + the element type ([abi] <type> ::= C
// <type>): Cf, Cd, Ce, and the integer forms, through pointers, references,
// templates and the imaginary-constant type (WP-211).
void f(_Complex float, _Complex double, _Complex long double) {}
void g(_Complex int, _Complex unsigned char, _Complex long long) {}
void p(const _Complex double *, _Complex float &, _Complex long double &&) {}
template <class T> void h(T) {}
template <class T> struct S { void m(T) {} };
void use() {
  h<_Complex double *>(nullptr);
  h(2.0i);
  h(1.5fi);
  S<_Complex long double>().m(0);
}
