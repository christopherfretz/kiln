// [temp.variadic]/5: expansion in a function parameter-type-list.
template <class... Ts> struct V { using F = void (*)(Ts...); };
static_assert(__is_same(V<int, char>::F, void (*)(int, char)));
static_assert(__is_same(V<>::F, void (*)()));
