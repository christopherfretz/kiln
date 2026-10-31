//@ expect: compile-ok
// [stmt.if]/2: a concept-id as an if constexpr condition.
template <class T> concept HasX = requires(T t) { t.x; };
struct P { int x = 5; };
template <class T> constexpr int get(T t) {
  if constexpr (HasX<T>)
    return t.x;
  else
    return 0;
}
static_assert(get(P{}) == 5);
static_assert(get(1) == 0);
