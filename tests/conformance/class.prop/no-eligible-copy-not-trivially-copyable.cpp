// [class.prop]/1, [special]/6: a class whose copy and move constructors
// and assignment operators are all deleted has no eligible one, so it is
// not trivially copyable; a volatile member of class type deletes all
// four (no constructor or assignment of `I` accepts a volatile `I`).
// [class.prop]/2: a class whose only default constructor is deleted (a
// const member with no initializer of a type that is not
// const-default-constructible, [class.default.ctor]/2.4) has no eligible
// default constructor and is not trivial. kiln answers true for all of
// these, as GCC 14 does. GCC 16.1 follows the standard on the second part
// only; clang 19/22 answer false on the first part (for its own reason,
// see const-class-member-trivially-copyable.cpp) and true for T1 and C9.
// Found by WP-1445.
//@ xfail: kiln counts deleted special members as eligible (no eligible copy/move member, no eligible default constructor)
//@ strict-diags
struct E {};
struct I { int i; };
struct C5 { volatile I i; };
struct C6 { const volatile E e; };
static_assert(!__is_trivially_copyable(C5));
static_assert(!__is_trivially_copyable(C6));

struct T1 { const int x; };
struct C2 { const I i; int j; };
struct C9 { C2 c; };
static_assert(!__is_trivial(T1));
static_assert(!__is_trivial(C2));
static_assert(!__is_trivial(C9));
