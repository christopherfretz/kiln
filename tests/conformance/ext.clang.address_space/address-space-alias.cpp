//@ expect: compile-ok
//@ strict-diags
// The decl-specifier address space of an alias's type-id qualifies the
// type its specifiers name, not the whole type (WP-1811): `int AS *` is
// a pointer to `AS int`, `AS int (*)()` returns `AS int`, and on a
// function type it is accepted as clang does.
#define AS __attribute__((address_space(1)))
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;

using t1 = int AS *;
static_assert(same_v<t1, AS int *>);
static_assert(!same_v<t1, int *AS>);
using t2 = AS int (*)();
static_assert(same_v<t2, AS int (*)()>);
static_assert(!same_v<t2, int (*)()>);
using t3 = void AS (*)();
typedef void AS t4();
using t5 = void AS ();
template <int N> using t6 = void __attribute__((address_space(N))) ();
t6<2> *f6;
