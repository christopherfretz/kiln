// [dcl.attr] (P3475R2, C++26): the carries_dependency attribute is removed,
// so `__has_cpp_attribute(carries_dependency)` is 0 (GCC 16.1 agrees) and
// `[[carries_dependency]]` is an unknown attribute, warned about as clang
// warns about any unknown attribute; its argument list is skipped with it
// (GCC 16.1, which still knows the attribute, rejects one). Earlier
// standards are unchanged (carries-dependency-cxx23.cpp).
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P3475R2
//@ diag: 16:3 warning "unknown attribute 'carries_dependency' ignored [-Wunknown-attributes]"
//@ diag: 17:3 warning "unknown attribute 'carries_dependency' ignored [-Wunknown-attributes]"
//@ diag: 18:10 warning "unknown attribute 'carries_dependency' ignored [-Wunknown-attributes]"
static_assert(__has_cpp_attribute(carries_dependency) == 0);
static_assert(__has_cpp_attribute(__carries_dependency__) == 0);
[[carries_dependency]] void f();
[[carries_dependency(1)]] void g();
void h([[carries_dependency]] int *);
