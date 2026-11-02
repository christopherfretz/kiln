// [expr.cond]/7.3, [expr.type]/4.6: when both operands are pointers to
// members (or one is a null pointer constant), the result has their
// composite pointer type: the cv-combined pointee in the more derived
// class, with a pointer to noexcept member function converting to the
// non-noexcept one (libc++ common_with, common_reference,
// equality_comparable_with).
//@ expect: compile-ok
//@ strict-diags
struct S { int m; int f(); };
struct D : S {};
bool b;
using PM = int S::*;
using CPM = const int S::*;
PM pm;
CPM cpm;
int D::*dm;
int (S::*pf)();
int (S::*pnf)() noexcept;
static_assert(__is_same(decltype(b ? pm : cpm), const int S::*));
static_assert(__is_same(decltype(b ? cpm : pm), const int S::*));
static_assert(__is_same(decltype(b ? pm : nullptr), int S::*));
static_assert(__is_same(decltype(b ? 0 : cpm), const int S::*));
static_assert(__is_same(decltype(b ? &S::m : cpm), const int S::*));
static_assert(__is_same(decltype(b ? pf : pnf), int (S::*)()));
static_assert(__is_same(decltype(b ? pm : dm), int D::*));
template <class T, class U> concept common = requires(bool c, T t, U u) { c ? t : u; };
static_assert(common<int S::*, const int S::*>);
static_assert(!common<int S::*, double S::*>);
