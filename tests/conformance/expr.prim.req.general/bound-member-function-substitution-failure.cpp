// [expr.prim.req.general]/5, [expr.mptr.oper]/6: `t.*pmf` and `p->*pmf`
// naming a member function can only be the operand of a call, so as a
// simple requirement they are a substitution failure (the requirement is
// not satisfied), while a call through them, or a pointer to data member,
// satisfies it. Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-ok
template <class M, class T>
concept DotStar = requires(M m, T t) { t.*m; };
template <class M, class T>
concept ArrowStar = requires(M m, T* p) { p->*m; };
template <class M, class T>
concept CallDotStar = requires(M m, T t) { (t.*m)(); };
template <class M, class T>
concept CallArrowStar = requires(M m, T* p) { (p->*m)(); };

struct A {
  int f();
  int d;
};

static_assert(!DotStar<int (A::*)(), A>);
static_assert(!ArrowStar<int (A::*)(), A>);
static_assert(CallDotStar<int (A::*)(), A>);
static_assert(CallArrowStar<int (A::*)(), A>);
static_assert(DotStar<int A::*, A>);
static_assert(ArrowStar<int A::*, A>);
static_assert(!CallDotStar<int A::*, A>);

// Overload resolution falls back when the constraint is not satisfied.
template <class D, class T>
constexpr int invoke(D(T::*pm), T&& t)
  requires requires { t.*pm; }
{ return 1; }
constexpr int invoke(...) { return 2; }
static_assert(invoke(&A::f, A()) == 2);
static_assert(invoke(&A::d, A()) == 1);
