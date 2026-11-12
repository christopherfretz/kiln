// [depr.arith.conv.enum]: as in clang, a comparison or conditional expression
// between enumerations either of which has no name for linkage warns under
// -Wdeprecated-anon-enum-enum-conversion, not -Wdeprecated-enum-compare(-conditional),
// so disabling the former silences it. Reduced from the reference workload: static
// assertions comparing `Trait<T>::VALUE` (an unnamed enum per instantiation).
//@ std: c++23
//@ expect: compile-fail
//@ args: -Werror -Wall -Wenum-compare-conditional -Wno-deprecated-anon-enum-enum-conversion
//@ strict-diags
//@ diag: 22:22 error "comparison of different enumeration types ('TE' and 'E1') is deprecated"
//@ diag: 23:26 error "conditional expression between different enumeration types ('E1' and 'E2') is deprecated"
template <class T> struct A {
    enum { VALUE = alignof(T) };
};
static_assert(A<int>::VALUE == A<float>::VALUE);
enum { X = 1 };
enum E1 { e1 };
enum E2 { e2 };
typedef enum { T1 } TE;
bool f() { return e1 == X; }
int g(bool b) { return b ? e1 : X; }
bool t() { return T1 == e1; }
int d(bool b) { return b ? e1 : e2; }
