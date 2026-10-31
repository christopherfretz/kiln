// [dcl.type.pack.index]: a pack-index-specifier in a class template member
// and as a function return type.
template <class... Ts> struct tl {
  using front = Ts...[0];
  using back = Ts...[sizeof...(Ts) - 1];
};
using F = tl<short, int, long>::front;
using B = tl<short, int, long>::back;
static_assert(__is_same(F, short));
static_assert(__is_same(B, long));
template <class... Ts> Ts...[0] head(Ts... ts) { return ts...[0]; }
static_assert(__is_same(decltype(head(1.0, 2)), double));
