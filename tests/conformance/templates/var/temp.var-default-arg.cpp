// [temp.param]/10, [temp.spec.partial.match]: default template arguments
// of a variable template, and a partial specialization matching them.
template <class T, class U = T> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<int>);
static_assert(same<int, int>);
static_assert(!same<int, long>);
template <class T, int N = sizeof(T)> constexpr int bytes = N;
static_assert(bytes<long> == 8);
static_assert(bytes<long, 2> == 2);
