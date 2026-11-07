// [dcl.attr.depend] (C++23): carries_dependency is a standard attribute,
// `__has_cpp_attribute(carries_dependency)` is 200809L, it appertains to a
// function or a parameter and takes no arguments. P3475R2 removes it in
// C++26 (carries-dependency-removed-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:21 error "attribute 'carries_dependency' cannot have an argument list"
static_assert(__has_cpp_attribute(carries_dependency) == 200809L);
[[carries_dependency]] void f();
void h([[carries_dependency]] int *);
[[carries_dependency(1)]] void g();
