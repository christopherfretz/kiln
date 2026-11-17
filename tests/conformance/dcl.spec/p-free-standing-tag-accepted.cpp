// [dcl.spec]: specifiers on a class declaration that also declares an
// object, or a linkage specification on it, apply to the object.
//@ expect: compile-ok
//@ strict-diags
const struct S5 { int a; } s5 = {1};
static struct S7 { int a; } s7;
extern "C" struct S8;
extern "C" { struct S9 { int b; }; }
constexpr struct S10 { int a; } s10 = {2};
typedef struct { int x; } T;
inline struct S11 { int a; } s11;
static union { int x; };
int use() { return s5.a + s7.a + s10.a + s11.a + x + T{3}.x; }
