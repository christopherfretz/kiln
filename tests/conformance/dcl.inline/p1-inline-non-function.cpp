// [dcl.inline]/1: the inline specifier applies only to functions and
// variables: not to a non-static data member, a parameter, a typedef, an
// alias, a block-scope variable or a class or enumeration declaration.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:12 error "'inline' can only appear on functions and non-local variables"
//@ diag: 26:9 error "'inline' can only appear on functions and non-local variables"
//@ diag: 27:1 error "'inline' can only appear on functions and non-local variables"
//@ diag: 28:1 error "'inline' can only appear on functions and non-local variables"
//@ diag: 29:12 error "inline declaration of 'x' not allowed in block scope"
//@ diag: 29:37 error "inline declaration of 'y' not allowed in block scope"
//@ diag: 31:1 error "'inline' can only appear on functions and non-local variables"
//@ diag: 32:1 error "'inline' can only appear on functions and non-local variables"
//@ diag: 33:31 error "'inline' can only appear on functions and non-local variables"
//@ diag: 34:8 error "expected unqualified-id"
//@ diag: 35:9 error "'inline' can only appear on functions and non-local variables"
//@ diag: 35:16 error "'virtual' can only appear on non-static member functions"
//@ diag: 35:24 error "'explicit' can only appear on non-static member functions"
//@ diag: 36:19 error "'virtual' can only appear on non-static member functions"
//@ diag: 36:12 error "'inline' can only appear on functions and non-local variables"
//@ diag: 37:1 error "'inline' can only appear on functions and non-local variables"
//@ diag: 37:8 error "'virtual' can only appear on non-static member functions"
//@ diag: 38:15 error "'inline' can only appear on functions and non-local variables"
struct c { inline int i; inline static int s = 1; };
int foo(inline int i);
inline class c2;
inline typedef int t;
void g() { inline int x = 0; static inline int y = 0; }
inline struct S3 { int a; } s3;
inline struct S4 { int a; };
inline enum E { e1 };
template<class T> struct TT { inline T m; };
inline using U = int;
void f1(inline virtual explicit int a);
struct S { inline virtual int m; };
inline virtual struct Q {};
inline int f2(inline int);
