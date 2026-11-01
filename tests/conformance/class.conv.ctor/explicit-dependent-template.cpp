//@ expect: run
// [dcl.fct.spec]/3, [temp.deduct.general]/7: the explicit-specifier of a
// constructor template is substituted with the deduced arguments, so a
// value-dependent `explicit(expr)` is resolved per specialization: an
// `explicit(false)` specialization is a converting constructor in a
// copy-initialization ([over.match.copy]/1). A member of a class
// template substitutes it with the class (libc++ `tuple`).
template <bool B> struct BC { static constexpr bool value = B; };
template <class T, class U> struct Conv : BC<__is_convertible(T, U)> {};
template <class... B> struct And : BC<(B::value && ...)> {};
template <class... T> struct Tup {
  int k = 0;
  Tup(T...) {}
  template <class... U, int = 0>
  explicit(!And<Conv<U, T>...>::value) Tup(Tup<U...>&&) : k(1) {}
};
struct Exp {
  template <class T> explicit(sizeof(T) > 1) Exp(T) {}
};
template <class T> struct W {
  explicit(sizeof(T) > 1) W(int) {}
};
Tup<int, long> f(int& a, long& b) { return Tup<int&, long&>(a, b); }
template <class T> concept CopyInit = requires(void (*g)(T), int i) { g(i); };
template <class T> concept CopyInitC = requires(void (*g)(T), char c) { g(c); };
static_assert(CopyInitC<Exp>);
static_assert(!CopyInit<Exp>);
static_assert(CopyInit<W<char>>);
static_assert(!CopyInit<W<int>>);
int main() {
  int a = 1;
  long b = 2;
  Tup<int, long> t = f(a, b);
  Exp e = 'c';
  Exp e2(3);
  W<char> w = 3;
  (void)e; (void)e2; (void)w;
  return t.k == 1 ? 0 : 1;
}
