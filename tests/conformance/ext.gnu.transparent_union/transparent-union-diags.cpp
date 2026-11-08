// GNU transparent_union in C++ (WP-809). clang ignores every occurrence with
// a warning; a class-head one also goes through clang's delayed subject and
// field checks, and if those pass the union passes by value as its first
// field (tests/codegen/transparent-union.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:22 warning "attribute ignored"
//@ diag: 51:18 error "no matching function for call to 'f'"
//@ diag: 50:6 note "candidate function not viable: no known conversion from 'int *' to 'U' for 1st argument"
//@ diag: 52:53 warning "attribute ignored"
//@ diag: 53:44 warning "attribute ignored"
//@ diag: 54:9 warning "attribute ignored"
//@ diag: 55:41 warning "attribute ignored"
//@ diag: 55:41 warning "attribute only applies to unions"
//@ diag: 57:23 warning "attribute ignored"
//@ diag: 57:23 warning "attribute only applies to unions"
//@ diag: 58:9 warning "attribute ignored"
//@ diag: 58:9 warning "attribute only applies to unions"
//@ diag: 59:41 warning "attribute ignored"
//@ diag: 59:41 warning "attribute only applies to unions"
//@ diag: 60:22 warning "attribute ignored"
//@ diag: 60:22 warning "transparent_union attribute can only be applied to a union definition; attribute ignored"
//@ diag: 62:22 warning "attribute ignored"
//@ diag: 62:22 warning "transparent_union attribute can only be applied to a union definition; attribute ignored"
//@ diag: 63:21 warning "attribute ignored"
//@ diag: 65:22 warning "attribute ignored"
//@ diag: 65:22 warning "transparent union definition must contain at least one field; transparent_union attribute ignored"
//@ diag: 66:22 warning "attribute ignored"
//@ diag: 66:52 warning "first field of a transparent union cannot have floating point type 'float'; transparent_union attribute ignored"
//@ diag: 67:22 warning "attribute ignored"
//@ diag: 67:63 warning "first field of a transparent union cannot have floating point type '_Complex double'; transparent_union attribute ignored"
//@ diag: 69:22 warning "attribute ignored"
//@ diag: 69:49 warning "first field of a transparent union cannot have vector type 'v4' (vector of 4 'int' values); transparent_union attribute ignored"
//@ diag: 70:22 warning "attribute ignored"
//@ diag: 70:58 warning "size of field 'b' (8 bits) does not match the size of the first field in transparent union; transparent_union attribute ignored"
//@ diag: 70:50 note "size of first field is 32 bits"
//@ diag: 71:22 warning "attribute ignored"
//@ diag: 71:60 warning "size of field 'd' (64 bits) does not match the size of the first field in transparent union; transparent_union attribute ignored"
//@ diag: 71:50 note "size of first field is 32 bits"
//@ diag: 72:22 warning "attribute ignored"
//@ diag: 72:62 warning "alignment of field 'l' (64 bits) does not match the alignment of the first field in transparent union; transparent_union attribute ignored"
//@ diag: 72:51 note "alignment of first field is 8 bits"
//@ diag: 74:22 warning "attribute ignored"
//@ diag: 75:22 warning "attribute ignored"
//@ diag: 76:45 warning "attribute ignored"
//@ diag: 77:46 warning "attribute ignored"
// Kept: a by-value parameter passes as the first field, but C++ makes no
// conversion to the union.
union __attribute__((transparent_union)) U { int *i; long l; };
void f(U u);
void g(int *p) { f(p); }
typedef union { int *i; long l; } TY __attribute__((transparent_union));
union Z { int *i; long l; } __attribute__((transparent_union));
union [[gnu::transparent_union]] Z5 { int *i; long q; };
template <class T> union __attribute__((transparent_union)) TU { T *i; long l; };
// Not a union, a class template, or not a definition.
struct __attribute__((transparent_union)) G { int a; };
class [[gnu::transparent_union]] G2 { int a; };
template <class T> union __attribute__((transparent_union)) TT;
union __attribute__((transparent_union)) Fw;
union Fw2 { int a; };
union __attribute__((transparent_union)) Fw2;
enum __attribute__((transparent_union)) En { en };
// Field checks.
union __attribute__((transparent_union)) E {};
union __attribute__((transparent_union)) I { float a; int *b; };
union __attribute__((transparent_union)) Cx { _Complex double c; int *b; };
typedef int v4 __attribute__((vector_size(16)));
union __attribute__((transparent_union)) V { v4 v; int *b; };
union __attribute__((transparent_union)) J { int a; char b; };
union __attribute__((transparent_union)) F { int a; double d; };
union __attribute__((transparent_union)) A { char c[8]; long l; };
// Other positions are only ignored.
union __attribute__((transparent_union(1))) H { int *a; };
int x __attribute__((transparent_union));
union Z2 { int *i; char c; } __attribute__((transparent_union));
typedef union { float f; } TF __attribute__((transparent_union));
