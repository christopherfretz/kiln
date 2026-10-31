// [temp.spec.partial.general]/2, [temp.over.link]/6: a partial
// specialization redeclared with an equivalent constraint (different
// parameter name) is the same partial specialization.
template <class T> struct X { static constexpr int v = 0; };
template <class T> requires (sizeof(T) > 1) struct X<T *>;
template <class U> requires (sizeof(U) > 1) struct X<U *> { static constexpr int v = 1; };
static_assert(X<int *>::v == 1);
static_assert(X<char *>::v == 0);
