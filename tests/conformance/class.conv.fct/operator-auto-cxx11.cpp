// [class.conv.fct]/9 (C++14): a conversion function with a deduced return
// type ('operator auto()') is C++14; in C++11 it is ill-formed, and the
// invalid conversion function is no candidate. GNU '__auto_type' is never a
// conversion type (clang `GetDeclSpecTypeForDeclarator`, ConversionId;
// WP-1134).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 15:21 error "'auto' not allowed in conversion function type"
//@ diag: 16:18 error "no viable conversion from returned value of type 'S' to function return type 'int'"
//@ diag: 17:30 warning "'decltype(auto)' type specifier is a C++14 extension"
//@ diag: 17:21 error "'decltype(auto)' not allowed in conversion function type"
//@ diag: 18:21 error "'__auto_type' not allowed in conversion function type"
//@ diag: 19:21 error "'__auto_type' not allowed in conversion function type"
struct S { operator auto() { return 1; } };
int h() { return S(); }
struct D { operator decltype(auto)() { return 1; } };
struct G { operator __auto_type() { return 1; } };
struct H { operator __auto_type(); };
