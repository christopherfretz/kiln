// Coverage comparison program (scripts/coverage-compare.py): expressions
// in a function's written signature. clang's region-counter walker
// (`MapRegionCounters`, a RecursiveASTVisitor over the whole declaration)
// also visits the return type's template arguments, `decltype` operands,
// `noexcept` operands and default arguments, so they feed the function
// hash (and a `?:` or `&&` there takes a counter that never runs); kiln
// walks the body only (KNOWN in coverage-compare.py, a WP-941 follow-up).
#include <type_traits>
template <class T> struct is_big { static constexpr bool value = sizeof(T) > 4; };
template <class T>
auto pick(T x) -> std::enable_if_t<!is_big<T>::value, decltype(x + 1)> {
  return x + 1;
}
template <class T>
auto pick(T x) -> std::enable_if_t<is_big<T>::value == true, T> {
  return x;
}
int def(int a, int b = sizeof(int) > 2 ? 1 : 2) { return a + b; }
void nx(int) noexcept(sizeof(int) == 4 && true) {}
int plain(int a) { return a ? 1 : 0; }
int main() {
  nx(1);
  return pick(1) + int(pick(2LL)) + def(1) + plain(0) - 8;
}
