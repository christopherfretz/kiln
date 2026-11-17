// [intro.execution]/5, [class.temporary]/4, [dcl.fct.default]/1: a
// template argument is a constant-expression, a full-expression of its own,
// so the temporaries it creates (here `Nil{}` and `L{N}`, copied from a
// temporary before C++17) end with it, even when it is dependent and is
// converted only where it is substituted, or is never converted in the
// template (an argument of a dependent template name or of a parameter of
// dependent type). The next full-expression of the declaration (a dependent
// braced default argument `= {}`, a default member initializer, a lambda)
// does not take them. Reduced from real code (range-v3's C++14 constraint
// emulation); siblings at each language level (WP-1578).
//@ args: -std=c++20
template <bool B, class T> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
struct Nil {};
struct L { int v; };
constexpr bool truefn(Nil) { return true; }
constexpr int one(L l) { return l.v; }
template <int N> struct I { static constexpr int value = N; };

// In a default template argument, then a braced default argument.
template <int N, class R = typename enable_if<truefn(Nil{}) && N == 0, Nil>::type>
int f1(R = {}) { return 1; }
int x1 = f1<0>();

// In an alias template.
template <int N> using A = typename enable_if<truefn(Nil{}) && N == 0, Nil>::type;
template <int N = 0> int f2(A<N> = {}) { return 2; }
int x2 = f2();

// In a noexcept condition.
template <int N = 0>
int f3(Nil = {}, typename enable_if<N == 0, Nil>::type = {}) noexcept(one(L{N})) { return 3; }
int x3 = f3();

// In an array bound.
template <int N = 0>
int f4(int (*)[one(L{N + 1})] = nullptr, typename enable_if<N == 0, Nil>::type = {}) { return 4; }
int x4 = f4();

// Followed by default member initializers.
template <int N> struct M {
  typename enable_if<truefn(Nil{}) && N == 0, Nil>::type a = {};
  int b = I<one(L{N})>::value;
  Nil c = {};
};
M<0> m;

// Followed by a lambda.
template <int N = 0>
int f5(typename enable_if<truefn(Nil{}) && N == 0, Nil>::type = {},
       int (*g)() = [] { return 5; }) { return g(); }
int x5 = f5();

// A non-dependent argument of a dependent template name, never converted
// in the template.
template <class T>
int f6(typename T::template X<one(L{1})>::type = {}, Nil = {}) { return 6; }
struct HasX { template <int> struct X { using type = int; }; };
int x6 = f6<HasX>();

// A non-dependent argument for a parameter of dependent type.
template <class T, T V> struct V1 { using type = Nil; };
template <class T>
int f7(typename V1<T, one(L{7})>::type = {}, Nil = {}) { return 7; }
int x7 = f7<int>();

// An explicit condition, and a temporary whose constexpr destructor is not
// trivial ([class.temporary]/4: destroyed at the end of the argument's
// evaluation).
struct CD {
  int v;
  constexpr CD(int i) : v(i) {}
  constexpr ~CD() {}
};
constexpr int cd(CD c) { return c.v; }
template <class T> struct E {
  template <int N = 0>
  explicit(cd(CD{N}) != 0) E(typename enable_if<cd(CD{N}) == 0, Nil>::type = {}) {}
};
E<int> e = {};
template <int N = 0>
int f8(typename enable_if<cd(CD{N}) == 0, Nil>::type = {}, Nil = {}) { return 8; }
int x8 = f8();
