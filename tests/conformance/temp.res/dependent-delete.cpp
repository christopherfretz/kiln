// [expr.delete], [temp.dep.expr]: a delete-expression with a type-dependent operand is checked
// when instantiated, also in an unevaluated operand (libc++'s __is_deletable)
//@ expect: run
//@ strict-diags
template <class T> T&& declval();
template <class P, class = void> struct is_deletable { static constexpr bool value = false; };
template <class P> struct is_deletable<P, decltype(delete declval<P>())> {
  static constexpr bool value = true;
};
template <class P, class = void> struct is_array_deletable { static constexpr bool value = false; };
template <class P> struct is_array_deletable<P, decltype(delete[] declval<P>())> {
  static constexpr bool value = true;
};
int destroyed = 0;
struct S { ~S() { ++destroyed; } };
static_assert(is_deletable<int*>::value);
static_assert(!is_deletable<int>::value);
static_assert(is_array_deletable<S*>::value);
static_assert(!is_array_deletable<S>::value);
template <class T> void destroy(T* p) { delete p; }
template <class T> void destroy_array(T p) { delete[] p; }
template <class T> void global_destroy(T p) { ::delete p; }
int main() {
  destroy(new S);
  destroy_array(new int[2]);
  global_destroy(new S);
  return destroyed == 2 ? 0 : 1;
}
