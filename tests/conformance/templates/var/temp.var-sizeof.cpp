// [temp.pre]/1, [temp.res.general]: a variable template whose initializer
// depends on the parameter; each specialization has its own value.
template <class T> constexpr unsigned long size_of = sizeof(T);
template <class T> constexpr unsigned long align_of = alignof(T);
struct S { char c; int i; };
static_assert(size_of<char> == 1);
static_assert(size_of<int> == 4);
static_assert(size_of<S> == 8);
static_assert(align_of<S> == 4);
static_assert(size_of<S[3]> == 24);
