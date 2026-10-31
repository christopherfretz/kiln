// [temp.over.link]/5: `sizeof(T)` and `sizeof(U)` naming the parameter in
// the same position are equivalent, so the alias template is redeclared.
template <class T> using A = int[sizeof(T) + 1];
template <class U> using A = int[sizeof(U) + 1];
static_assert(sizeof(A<int>) == sizeof(int) * (sizeof(int) + 1));

template <int N> using B = char[N * 2];
template <int M> using B = char[M * 2];
static_assert(sizeof(B<3>) == 6);
