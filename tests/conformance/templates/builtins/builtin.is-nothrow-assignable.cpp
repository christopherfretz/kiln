// [meta.unary.prop] is_nothrow_assignable (clang's __is_nothrow_assignable):
// declval<T>() = declval<U>() is well-formed as an unevaluated operand and
// cannot throw. libc++ uses the builtin directly.
//@ expect: compile-ok
//@ strict-diags
struct A { A& operator=(const A&) noexcept; A& operator=(int); };
struct D { D& operator=(const D&) = delete; };
static_assert(__is_nothrow_assignable(int&, int));
static_assert(!__is_nothrow_assignable(int, int));
static_assert(__is_nothrow_assignable(A&, const A&));
static_assert(__is_nothrow_assignable(A&, A));
static_assert(!__is_nothrow_assignable(A&, int));
static_assert(__is_assignable(A&, int));
static_assert(!__is_nothrow_assignable(D&, const D&));
static_assert(!__is_nothrow_assignable(void, int));
static_assert(__is_nothrow_assignable(const char*&, const char*));
template<class T> constexpr bool nma = __is_nothrow_assignable(T&, T&&);
static_assert(nma<const char*> && nma<A> && !nma<D>);
int main() {}
