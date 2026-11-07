// [except.spec]/6, [expr.unary.noexcept]/3: a new-expression is
// potentially-throwing through its allocation function or its
// initialization, so `::new (p) T(...)` through the non-throwing
// placement `operator new(size_t, void*)` is not when T's constructor is
// not; this is the operand of the noexcept-specifier of the standard
// allocator's `construct`. A delete-expression is potentially-throwing
// through the destroyed type's destructor or the deallocation function.
// kiln treated every new-expression as potentially-throwing and looked
// at no delete-expression's destructor (WP-783).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-unevaluated-expression
#include <new>
#include <string>
#include <utility>
struct S { S(S &&) noexcept; S(); };
struct T { T() noexcept; };
struct I { int a; std::string s; };
struct D { D(D &&) noexcept; ~D() noexcept(false); };
struct E {};
void *p;
static_assert(noexcept(::new (p) int(1)));
static_assert(noexcept(::new (p) S(std::declval<S>())));
static_assert(!noexcept(::new (p) S()));
static_assert(!noexcept(::new (p) S));
static_assert(noexcept(::new (p) T));
static_assert(noexcept(::new (p) I));
static_assert(noexcept(::new (p) std::string(std::declval<std::string>())));
static_assert(noexcept(::new (p) std::string()));
// The destructor of the new object is not invoked by the new-expression.
static_assert(noexcept(::new (p) D(std::declval<D>())));
static_assert(!noexcept(new int));
static_assert(noexcept(new (std::nothrow) int));
static_assert(!noexcept(new (std::nothrow) S));
static_assert(noexcept(new (std::nothrow) T[4]));
D *dp;
E *ep;
I *ip;
int *xp;
T *tp;
static_assert(!noexcept(delete dp));
static_assert(!noexcept(delete[] dp));
static_assert(noexcept(delete ep));
static_assert(noexcept(delete ip));
static_assert(noexcept(delete xp));
static_assert(noexcept(delete tp));
template <class X>
constexpr bool nothrow_relocate = noexcept(::new (std::declval<void *>()) X(std::declval<X>()));
static_assert(nothrow_relocate<std::string> && nothrow_relocate<int> && nothrow_relocate<S>);
template <class X> struct Thrower { Thrower(Thrower &&); };
static_assert(!nothrow_relocate<Thrower<int>>);
// An implicit destructor is potentially-throwing through a subobject's.
struct N { D d; };
template <class X> struct H { X x; };
N *np;
H<D> *hdp;
H<int> *hip;
static_assert(!noexcept(delete np));
static_assert(!noexcept(delete hdp));
static_assert(noexcept(delete hip));
