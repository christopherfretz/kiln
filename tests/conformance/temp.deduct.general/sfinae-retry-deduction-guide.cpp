// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature failed CTAD is re-deduced once a deduction guide is
// declared later.
//@ expect: compile-ok
template <class T> struct Box {
  Box(int);
};
template <class T> auto g(T t) -> decltype(Box(t), 0);
long g(...);

long p = sizeof(g(0));
static_assert(sizeof(g(0)) == sizeof(long));

Box(int) -> Box<int>;

static_assert(sizeof(g(0)) == sizeof(int));
