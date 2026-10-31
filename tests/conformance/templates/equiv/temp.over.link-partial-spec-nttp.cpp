// [temp.spec.partial.general]/2, [temp.over.link]/5: a partial
// specialization whose argument is an expression on its parameters is
// redeclared by an equivalent one.
template <int A, int B> struct P { static constexpr int v = 0; };
template <int N> struct P<N, N * 2>;
template <int M> struct P<M, M * 2> { static constexpr int v = 1; };
