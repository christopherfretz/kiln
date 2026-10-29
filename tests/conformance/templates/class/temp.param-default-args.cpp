//@ expect: ast
//@ oracle: clang
// [temp.param]/12: a default template argument completes the argument list.
template <class T, int N = 3> struct Vec { T data[N]; };
static_assert(sizeof(Vec<char>) == 3);
static_assert(sizeof(Vec<char, 5>) == 5);
