// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature substitution failed on an ADL call `h(t)` is
// re-deduced after `h` is declared in the associated namespace.
//@ expect: compile-ok
namespace N {
struct S {};
} // namespace N

template <class T> auto g(T t) -> decltype(h(t));
long g(...);

long p = sizeof(g(N::S{}));
static_assert(sizeof(g(N::S{})) == sizeof(long));

namespace N {
int h(S);
} // namespace N

static_assert(sizeof(g(N::S{})) == sizeof(int));
