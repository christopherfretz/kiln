// clang's using_if_exists: an unresolved using-declaration is fine until its
// name is used; a [[]] attribute on a using-declaration is an extension
//@ expect: compile-ok
//@ strict-diags
//@ diag: 8:14 warning "ISO C++ does not allow an attribute list to appear here"
//@ diag: 9:3 warning "ISO C++ does not allow an attribute list to appear here"
namespace N { int a; }
using N::a [[clang::using_if_exists]];
[[clang::using_if_exists]] using N::missing;
using N::gone __attribute__((__using_if_exists__)), N::a;
int k = a;
