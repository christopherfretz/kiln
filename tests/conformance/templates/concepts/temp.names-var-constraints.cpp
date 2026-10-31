//@ expect: compile-ok
// [temp.names]/8: a constrained variable template.
template <class T> requires (sizeof(T) == 4) constexpr int four = 4;
static_assert(four<int> == 4);
static_assert(four<float> == 4);
