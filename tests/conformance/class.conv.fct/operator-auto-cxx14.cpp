// [class.conv.fct]/9: from C++14 on a conversion function may have a deduced
// return type ('operator auto()', 'operator decltype(auto)()'), but GNU
// '__auto_type' is still no conversion type (clang `IsCXXAutoType`; WP-1134).
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ diag: 11:21 error "'__auto_type' not allowed in conversion function type"
struct S { operator auto() { return 1; } };
int h() { return S(); }
struct D { operator decltype(auto)() { return 2; } };
struct G { operator __auto_type(); };
