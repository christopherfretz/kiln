// Valid near-misses of the WP-948 undefined-behaviour checks: comparison and
// subtraction of one-past-the-end pointers ([expr.add]/4, [expr.rel]),
// non-virtual and null member-pointer comparison ([expr.eq]/4.1-2), a
// variable initialized after its initializer is seen ([expr.const]/2), the
// address of a volatile object, and a reference bound to the last element.
//@ std: c++26
//@ expect: compile-ok
struct S { int a, b; };
constexpr S s{1, 2};
constexpr int arr[4] = {1, 2, 3, 4};
static_assert(arr + 4 != arr + 3);
static_assert(arr + 4 == &arr[3] + 1);
static_assert(&arr[4] - arr == 4);
static_assert(&s.a + 1 != &s.a);
static_assert((arr + 3) - (arr + 1) == 2);
struct A { virtual int f(); int g(); int h(); };
static_assert(&A::g != &A::h);
static_assert(&A::g == &A::g);
static_assert(&A::f != nullptr);
constexpr int (A::*np)() = nullptr;
static_assert(np == nullptr);
extern const int x;
const int x = 3;
const int y = x;
static_assert(y == 3);
volatile constexpr int v = 1;
constexpr const volatile int *pv = &v;
static_assert(pv == &v);
constexpr int f() { int a[3]{}; int *e = a + 3; int &r = a[2]; return (e - a) + r; }
static_assert(f() == 3);
