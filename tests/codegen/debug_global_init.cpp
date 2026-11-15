// Debug info (WP-1445): the compiler-generated initialization functions
// get artificial DISubprograms as clang emits them: `__cxx_global_var_init`
// (named after itself, local to the unit), `__cxx_global_array_dtor` (an
// unnamed artificial `void *` parameter), `__tls_init` and
// `_GLOBAL__sub_I_*` (linkage name only); a guarded initializer's exit is
// at the variable. The thread wrapper has none, as in clang.
//@ args: -g
struct S { S(int); ~S(); int v; };
int f(int);
static int g = f(3);
S s(4);
struct N { N(int) noexcept; ~N(); };
N arr[2] = {1, 2};
thread_local S ts(5);
int use() { return ts.v + g; }
inline int ig = f(7);
template <class T> int tv = f(sizeof(T));
int ut() { return tv<int> + ig; }
