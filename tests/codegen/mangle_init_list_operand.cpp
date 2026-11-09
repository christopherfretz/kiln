// Itanium ABI <expression>: a braced-init-list operand of a dependent
// call is `il <expression>* E` (clang `mangleExpression`
// `InitListExprClass`), in a template argument (`W<mn({1, V...})>`) and
// in a decltype return type (`decltype(mn({t, t}))`). Before WP-933
// kiln could not mangle it (`std::min({...})` in an alias template
// argument). Compared with clang 19 and 22 (ir_vs_clang).
namespace std {
template <class E> class initializer_list {
  const E* b;
  decltype(sizeof 0) n;
public:
  constexpr const E* begin() const { return b; }
};
template <class T> constexpr T mn(initializer_list<T> l) { return *l.begin(); }
}
template <int N> struct W {};
template <int... V> W<std::mn({1, V...})> f() { return {}; }
template <class T> auto g(T t) -> decltype(std::mn({t, t})) { return t; }
int use() {
  f<2, 3>();
  return g(1);
}
