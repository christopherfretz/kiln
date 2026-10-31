//@ expect: compile-ok
// [dcl.decl.general]/4: a trailing requires-clause constrains a function
// template.
template <class T> constexpr int h(T) requires (sizeof(T) > 4) { return 8; }
template <class T> constexpr int h(T) requires (sizeof(T) <= 4) { return 4; }
static_assert(h(1) == 4);
static_assert(h(1.0) == 8);
