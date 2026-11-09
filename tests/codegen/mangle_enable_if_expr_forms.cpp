// [temp.over.link]/5, Itanium ABI <expression>: an enable_if-style
// condition that depends on a template parameter is mangled as written,
// so function templates that differ only in that condition are distinct
// symbols. Forms WP-933 added (they collided as `XLi0EE` before):
// floating literals (`Ld...E`, `Lf...E`), `throw` and rethrow (`tw`,
// `tr`), `typeid` (`ti`), and temporaries (`cv`/`tl` constructions seen
// through their materialization). `sizeof("abc")` folds to `Lm4E`. Any
// form still without an encoding is a hard error (WP-593). Compared with clang 19 and 22 (ir_vs_clang).
namespace std { class type_info; }
template <bool B> struct E {};
template <> struct E<true> { using type = int; };
struct S {
  int v;
  constexpr S(unsigned long x) : v(int(x)) {}
};
struct P { unsigned long a, b; };
// Each pair: the first overload is chosen for `char`, the second for `int`.
template <class T> typename E<(sizeof(T) == 1 && 1.5 > 1.0)>::type f(T) { return 1; }
template <class T> typename E<(sizeof(T) != 1 && 2.5f > 1.0f)>::type f(T) { return 2; }
template <class T> typename E<sizeof(T) * 2 == sizeof("a")>::type g(T) { return 1; }
template <class T> typename E<sizeof(T) == sizeof("abc")>::type g(T) { return 2; }
template <class T> typename E<sizeof(T) == 1 && !noexcept(throw T())>::type h(T) { return 1; }
template <class T> typename E<sizeof(T) != 1 && !noexcept(throw)>::type h(T) { return 2; }
template <class T> typename E<sizeof(T) == 1 && noexcept(typeid(T))>::type i(T) { return 1; }
template <class T> typename E<sizeof(T) != 1 && noexcept(typeid(T()))>::type i(T) { return 2; }
template <class T> typename E<S(sizeof(T)).v == 1>::type j(T) { return 1; }
template <class T> typename E<P{sizeof(T), 2}.a == 4>::type j(T) { return 2; }
int use() {
  return f('a') + f(1) + g('a') + g(1) + h('a') + h(1) + i('a') + i(1) + j('a') + j(1);
}
