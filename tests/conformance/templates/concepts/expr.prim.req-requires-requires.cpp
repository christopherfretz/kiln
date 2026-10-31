//@ expect: compile-ok
// [temp.pre]/9: `requires requires` — an ad hoc requires-expression as a
// requires-clause.
template <class T> requires requires(T t) { t.size(); }
constexpr int len(T t) { return t.size(); }
template <class T> constexpr int len(T) { return -1; }
struct V { constexpr int size() const { return 3; } };
static_assert(len(V{}) == 3);
