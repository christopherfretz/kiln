// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature failed because a constexpr function was only declared
// is re-deduced once the constexpr function is defined.
//@ expect: compile-ok
constexpr int val();
template <int N> struct X {
  using t = int;
};
template <class T> auto g(T) -> typename X<val() + int(sizeof(T))>::t;
long g(...);

long p = sizeof(g(0));
static_assert(sizeof(g(0)) == sizeof(long));

constexpr int val() { return 1; }

static_assert(sizeof(g(0)) == sizeof(int));
