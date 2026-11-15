// [class.prop]/1, [special]/6: a class whose copy and move constructors
// and assignment operators are all deleted has no eligible one, so it is
// not trivially copyable; a volatile member of class type deletes all
// four (no constructor or assignment of `I` accepts a volatile `I`).
// [class.prop]/2: a class whose only default constructor is deleted (a
// const member with no initializer of a type that is not
// const-default-constructible, [class.default.ctor]/2.4) has no eligible
// default constructor and is not trivial. GCC 16.1 follows the standard on
// the second part only, clang 19/22 on the first part only (for their own
// reason, see const-class-member-trivially-copyable.cpp), so neither is the
// oracle (docs/status/deviations.md). Found by WP-1445, fixed by WP-1452.
//@ skip(clang): clang 19/22 count the deleted default constructor of T1, C2, C9 as eligible
//@ skip(gcc): GCC 16.1 makes a class whose copy/move operations are all deleted trivially copyable
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
