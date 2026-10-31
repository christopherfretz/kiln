// [meta.rel]/5 (clang's __is_convertible / __is_convertible_to /
// __is_nothrow_convertible): whether `To test() { return declval<From>(); }`
// is well-formed (copy-initialization, unevaluated, access from the TU).
// libc++'s is_convertible uses the builtin directly.
//@ expect: compile-ok
//@ strict-diags
struct A {}; struct B : A {}; struct E { explicit E(int); };
struct P { private: operator int(); };
struct C { C(int) noexcept; }; struct T { T(int); };
struct Inc;
static_assert(__is_convertible(int, long));
static_assert(__is_convertible(B*, A*));
static_assert(!__is_convertible(A*, B*));
static_assert(!__is_convertible(int, E));
static_assert(!__is_convertible(P, int));
static_assert(__is_convertible(void, void));
static_assert(__is_convertible(int, const int&));
static_assert(!__is_convertible(int, int&));
static_assert(__is_convertible(int&, int&));
static_assert(!__is_convertible(int[2], int[2]));
static_assert(__is_convertible(int(&)[2], int*));
static_assert(__is_convertible(void(), void(*)()));
static_assert(!__is_convertible(int, void()));
static_assert(__is_convertible_to(B, A));
static_assert(__is_nothrow_convertible(int, C));
static_assert(!__is_nothrow_convertible(int, T));
static_assert(__is_convertible(int, T));
static_assert(__is_convertible(Inc*, const void*));
template<class F, class To> constexpr bool cv = __is_convertible(F, To);
static_assert(cv<const char(&)[4], const char*>);
static_assert(!cv<const char*, int>);
static_assert(!__is_convertible(int, void));
int main() {}
