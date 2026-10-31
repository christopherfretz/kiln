// [temp.over.link]/5: template arguments `N + 1` and `M + 1` are
// equivalent, so the second declaration defines the first template.
template <int N> struct X {};
template <int N> int f(X<N + 1>);
template <int M> int f(X<M + 1>) { return M; }
int r = f<2>(X<3>());
