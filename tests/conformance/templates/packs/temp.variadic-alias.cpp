// [temp.variadic]/5: expansion in a template argument list of an alias.
template <class... Ts> struct tl {};
template <class... Ts> using wrap = tl<Ts...>;
template <class... Ts> using ptrs = tl<Ts *...>;
static_assert(__is_same(wrap<int, char>, tl<int, char>));
static_assert(__is_same(ptrs<int, char>, tl<int *, char *>));
static_assert(__is_same(wrap<>, tl<>));
