// [class.union.general]/6: one initialized variant member per union is
// valid, as are members of one anonymous struct, separate anonymous unions
// of a class, and an instantiated union template (WP-1015).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
union U1 { int a = 1; int b; };
union U2 { int a; union { int b; int c = 2; }; };
struct S3 { union { int a = 1; int b; }; union { int c; int d = 2; }; int e = 3; };
union U4 { struct { int a = 1; int b = 2; }; int c; };
template<class T> union U5 { T a = 1; T b; };
U5<int> u5;
union U6 { int : 3; int a = 1; };
constexpr U1 u1 = U1();
static_assert(u1.a == 1);
static_assert(U4{}.b == 2);
