// [class.prop]/1: a trivially copyable class has at least one eligible
// copy or move constructor or assignment operator, each eligible one
// trivial, and a trivial, non-deleted destructor; [special]/6: a deleted
// function is not eligible. A const member of class type deletes the
// implicit copy and move assignment ([class.copy.assign]/7), which are
// then not eligible, while the copy and move constructors stay trivial:
// the class is trivially copyable. [class.prop]/2: it is trivial when its
// default constructor is also eligible and trivial. Compared with GCC
// 16.1, which agrees; clang 19/22 count the deleted assignment as
// non-trivial (docs/status/deviations.md). Found by WP-1445.
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not count a class with a const member of class type as trivially copyable
//@ strict-diags
struct E {};
struct I { int i; };
struct M { I &operator=(const I &) const; };

struct C1 { const E e; };
struct C2 { const I i; int j; };
struct C3 { const E e[2]; };
struct C4 { const I i = {}; };
struct C7 { const M m; };
struct C8 : E { const E e; };
struct C9 { C2 c; };
static_assert(__is_trivially_copyable(C1) && __is_trivial(C1));
static_assert(__is_trivially_copyable(C2));
static_assert(__is_trivially_copyable(C3) && __is_trivial(C3));
// A default member initializer makes the default constructor non-trivial.
static_assert(__is_trivially_copyable(C4) && !__is_trivial(C4));
static_assert(__is_trivially_copyable(C7) && __is_trivial(C7));
static_assert(__is_trivially_copyable(C8) && __is_trivial(C8));
static_assert(__is_trivially_copyable(C9));

// The deleted assignment is neither trivially assignable nor assignable;
// the copy and move constructors are trivial.
static_assert(!__is_assignable(C1 &, const C1 &));
static_assert(!__is_trivially_assignable(C1 &, const C1 &));
static_assert(!__is_trivially_assignable(C1 &, C1 &&));
static_assert(__is_trivially_constructible(C1, const C1 &));
static_assert(__is_trivially_constructible(C1, C1 &&));
static_assert(!__is_trivially_assignable(C2 &, const C2 &));
static_assert(__is_trivially_constructible(C2, const C2 &));

// A const scalar, a mutable class member, a plain class member.
struct T1 { const int x; };
struct T2 { mutable I i; };
struct T3 { E e; I i; };
static_assert(__is_trivially_copyable(T1) && !__is_trivially_assignable(T1 &, const T1 &));
static_assert(__is_trivially_copyable(T2) && __is_trivial(T2));
static_assert(__is_trivially_copyable(T3) && __is_trivial(T3));
