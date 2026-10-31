// [temp.variadic]/5: expansion in a braced-init-list.
struct S { int a, b, c; };
template <class... Ts> constexpr S make(Ts... ts) { return S{ts...}; }
static_assert(make(1, 2, 3).b == 2);
template <class... Ts> constexpr int fst(Ts... ts) { int a[3] = {ts...}; return a[0]; }
static_assert(fst(7, 8, 9) == 7);
