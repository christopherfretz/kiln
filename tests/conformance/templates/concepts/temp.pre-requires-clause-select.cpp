//@ expect: compile-ok
// [temp.pre]/9: a requires-clause constrains the template.
template <class T> requires (sizeof(T) == 1) constexpr int g(T) { return 1; }
template <class T> requires (sizeof(T) != 1) constexpr int g(T) { return 2; }
static_assert(g('x') == 1);
static_assert(g(1.0) == 2);
