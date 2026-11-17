// [dcl.spec]: a declaration of only a class or enumeration takes no
// constexpr, consteval or constinit ([dcl.constexpr]/1), no function
// specifier, no typedef ([dcl.typedef]/1 "typedef requires a name") and no
// mutable ([dcl.stc]/1); storage classes and cv-qualifiers there are
// ignored with a warning (clang -Wmissing-declarations).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:1 error "class cannot be marked constexpr"
//@ diag: 36:1 error "struct cannot be marked consteval"
//@ diag: 37:1 error "union cannot be marked constinit"
//@ diag: 38:1 error "enum cannot be marked constexpr"
//@ diag: 39:1 error "enum class cannot be marked constexpr"
//@ diag: 40:1 error "enum struct cannot be marked constexpr"
//@ diag: 41:1 error "struct cannot be marked constexpr"
//@ diag: 42:1 error "'virtual' can only appear on non-static member functions"
//@ diag: 42:9 error "'explicit' can only appear on non-static member functions"
//@ diag: 43:1 error "'mutable' can only be applied to member variables"
//@ diag: 44:1 warning "typedef requires a name [-Wmissing-declarations]"
//@ diag: 45:1 warning "typedef requires a name [-Wmissing-declarations]"
//@ diag: 46:12 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 46:31 warning "'static' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 46:51 error "'mutable' can only be applied to member variables"
//@ diag: 47:1 warning "'static' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 48:1 warning "'extern' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 49:1 warning "'thread_local' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 50:1 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 50:1 warning "'volatile' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 51:1 warning "'static' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 51:8 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 52:1 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 53:1 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 54:1 warning "'const' is not permitted on a declaration of a type [-Wmissing-declarations]"
//@ diag: 55:19 warning "'static' is not permitted on a declaration of a type [-Wmissing-declarations]"
constexpr class A1 {};
consteval struct A2 {};
constinit union A3 {};
constexpr enum A4 {};
constexpr enum class A5 {};
constexpr enum struct A6 {};
constexpr struct A7;
virtual explicit struct A9;
mutable struct Q {};
typedef struct A;
typedef struct B {};
struct M { const struct N {}; static struct O {}; mutable struct P {}; };
static struct S1 {};
extern struct S2 {};
thread_local struct S3 {};
const volatile struct S4 {};
static const struct S6;
const class C { int x; };
const enum E { e };
const union U {};
template<class T> static struct TE;
