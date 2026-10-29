//@ expect: compile-ok
// [temp.arg.template]/1: an alias template used as a template template
// argument names its substituted type.
template <class T> using Ptr = T *;
template <template <class> class F> struct Apply { F<int> v; };
static_assert(sizeof(Apply<Ptr>) == sizeof(int *));
int x;
Apply<Ptr> a = {&x};
