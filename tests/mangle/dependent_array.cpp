// Mangling differential: array types whose bound is value-dependent in a
// function template signature (Itanium ABI §5.1.5 <array-type> ::=
// A <expression> _ <type>). These used to mangle as `u7unknown`, so the
// `const char (&)[N]` and `const wchar_t (&)[N]` overloads shared one
// symbol (WP-686).
struct M {
  template <int N> M &operator<<(const char (&)[N]) { return *this; }
  template <int N> M &operator<<(const wchar_t (&)[N]) { return *this; }
  template <unsigned N> void f(int (*)[N]) {}
};
template <class T, int N> void g(T (&)[N], T (&)[N]) {}
template <class T, int N> void h(T (*)[N + 1]) {}
template <class T> void k(char (*)[sizeof(T)]) {}
template <int N> void m(const char (&)[N][2]) {}

void use() {
  M o;
  o << "ab" << L"ab";
  int a[3];
  o.f(&a);
  g(a, a);
  int b[4][4];
  h<int, 3>(b);
  char c[3][4];
  k<int>(c);
  char d[2][2] = {};
  m<2>(d);
}
