// [over.ics.ref]: a reference parameter binding directly is the identity (or
// a derived-to-base conversion); a binding to a temporary is the conversion to
// the referenced type. Reference bindings never bind an rvalue reference to an
// lvalue or a non-const lvalue reference to an rvalue.
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
struct T1 {}; struct T2 {}; struct T3 {};
struct B {}; struct D : B {};
D d;
int i;
T1 r(B &);
T2 r(D &&);
static_assert(same<decltype(r(d)), T1>);
static_assert(same<decltype(r(D())), T2>);

T1 c(const long &);
T2 c(double);
T1 lr(int &);
T2 lr(...);
static_assert(same<decltype(lr(1)), T2>);   // no binding of int& to an rvalue
static_assert(same<decltype(lr(i)), T1>);
T1 rr(int &&);
T2 rr(...);
static_assert(same<decltype(rr(i)), T2>);   // no binding of int&& to an lvalue
static_assert(same<decltype(rr(1L)), T1>);  // a temporary int
T1 qr(int &);
T2 qr(const volatile int &);
const int ci = 1;
static_assert(same<decltype(qr(ci)), T2>);
T1 tb(const B &);
T2 tb(const D &);
static_assert(same<decltype(tb(d)), T2>);   // identity over derived-to-base
static_assert(same<decltype(tb(B())), T1>);
