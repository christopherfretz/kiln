// [dcl.pre]/12: the requirement text prints a cast to a reference with its & / && (WP-145)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion failed due to requirement 'static_cast<const Leaf<1, long> &>(x).v == 3'"
//@ diag: 18:54 note "expression evaluates to '2 == 3'"
//@ diag: 19:15 error "static assertion failed due to requirement 'static_cast<const Leaf<1, long> &&>(x).v == 3'"
//@ diag: 19:55 note "expression evaluates to '2 == 3'"
//@ diag: 20:15 error "static assertion failed due to requirement '((const Leaf<1, long> &)x).v == 3'"
//@ diag: 20:43 note "expression evaluates to '2 == 3'"
//@ diag: 21:15 error "static assertion failed due to requirement 'static_cast<const int (&)[1]>(a)[0] == 3'"
//@ diag: 21:50 note "expression evaluates to '2 == 3'"
//@ diag: 22:15 error "static assertion failed due to requirement 'static_cast<const Leaf<1, long> *>(&x)->v == 3'"
//@ diag: 22:56 note "expression evaluates to '2 == 3'"
template <int I, class T> struct Leaf { T v; constexpr Leaf(T x) : v(x) {} };
struct X : Leaf<1, long> { constexpr X() : Leaf<1, long>(2) {} };
constexpr X x;
constexpr int a[1] = {2};
static_assert(static_cast<const Leaf<1, long>&>(x).v == 3);
static_assert(static_cast<const Leaf<1, long>&&>(x).v == 3);
static_assert(((const Leaf<1, long>&)x).v == 3);
static_assert(static_cast<const int(&)[1]>(a)[0] == 3);
static_assert(static_cast<const Leaf<1, long>*>(&x)->v == 3);
