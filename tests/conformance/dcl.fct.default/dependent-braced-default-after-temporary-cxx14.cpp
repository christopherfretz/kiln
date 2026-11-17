// [dcl.fct.default]/1, [temp.inst]/12: a braced default argument `= {}` of
// a parameter whose type is dependent is kept as written and
// list-initializes the substituted type when the default is used. The
// parameter's type here holds `truefn(Nil{})`, a call whose argument is,
// before C++17, copied from a temporary ([class.copy.elision]/1.3 in
// C++14): that temporary does not change how the default is instantiated.
// Reduced from real code (range-v3's C++14 constraint emulation for
// constructors, `enable_if_t<C && CPP_true_fn(xNil{}), Nil> = {}`).
//@ args: -std=c++14
template <bool B, class T> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <bool B, class T> using enable_if_t = typename enable_if<B, T>::type;
struct Nil {};
constexpr bool truefn(Nil) { return true; }
template <class T> constexpr bool dc = __is_constructible(T);

template <int N = 0>
int f(typename enable_if<truefn(Nil{}) && N == 0, Nil>::type = {}) { return 1; }
int x = f();

template <class T>
struct M {
  template <bool (&F)(Nil) = truefn>
  constexpr M(enable_if_t<dc<T> && F(Nil{}), Nil> = {}) {}
};
M<int> m;
