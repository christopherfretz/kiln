// [temp.alias]/2, [temp.variadic]/5, [expr.sizeof]/5: `sizeof...(U)` in an
// alias template named with pack expansions (`N<Ts...>`) is not known
// until the expansions' packs are: it becomes `k + sizeof...(P1) + ...`
// (a value-dependent expression), so `N<Ts...>` is a dependent type and
// `N<Ts...>::value` is looked up per instantiation (clang's partially
// substituted SizeOfPackExpr).
template <class T, T v> struct ic { static constexpr T value = v; };
template <class... U> using N = ic<unsigned long, sizeof...(U)>;
template <int... U> using M = ic<unsigned long, sizeof...(U)>;
template <class... Ts> constexpr unsigned long f() { return N<Ts...>::value; }
static_assert(f<int, char, long>() == 3 && f<>() == 0);
template <class... Ts> constexpr unsigned long f2() {
  using Q = N<Ts...>;
  return sizeof(Q) + Q{}.value;
}
static_assert(f2<int, char, long>() == 4);
template <class... Ts> void f3() { (void)N<Ts...>::nope; } // dependent: not checked
template <class... Ts> constexpr unsigned long a() { return N<int, Ts *..., char>::value; }
static_assert(a<int, char, long>() == 5 && a<>() == 2);
template <int... Is> constexpr unsigned long b() { return M<Is..., 7, Is...>::value; }
static_assert(b<1, 2>() == 5);
template <class... Ts> constexpr unsigned long c(Ts... xs) { return M<sizeof(xs)...>::value; }
static_assert(c(1, 2.0, 'c') == 3);
template <class... Ts> struct Outer {
  template <class... Us> using In = N<Ts..., Us...>;
};
static_assert(Outer<int, int>::In<char>::value == 3);
