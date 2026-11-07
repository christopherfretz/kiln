// [temp.arg.nontype]/6, [temp.deduct.general]/8: a string literal
// explicitly specified for an `auto` template parameter is an invalid
// argument, so that candidate is dropped before its constraints are
// checked, and a class-type parameter of another overload takes the
// literal (reflect-cpp's `make_field<"name">` beside
// `make_field<&C::member>`).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2
extern "C" int printf(const char*, ...);
template <unsigned N> struct Lit {
  constexpr Lit(const char (&s)[N]) {
    for (unsigned i = 0; i < N; ++i) a[i] = s[i];
  }
  char a[N]{};
};
template <class T> constexpr bool is_mp = false;
template <class M, class C> constexpr bool is_mp<M C::*> = true;
template <Lit n, class T> int make(T&&) { return 1; }
template <auto p, class T>
  requires(is_mp<decltype(p)>)
int make(T&&) { return 2; }
struct P { int x; };
int main() { printf("%d %d\n", make<"ab">(1), make<&P::x>(1)); }
