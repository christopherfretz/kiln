//@ expect: ast
//@ oracle: clang
// [temp.param]/12: a default template argument may refer to an earlier
// parameter; it is substituted with the arguments before it.
template <class T, class U = T *> struct P { T t; U u; };
static_assert(sizeof(P<char>) == 2 * sizeof(void *));
