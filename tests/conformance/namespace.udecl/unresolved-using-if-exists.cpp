// clang's using_if_exists: a missing target is ignored by lookup, but a
// use of the name is an error
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:3 warning "ISO C++ does not allow an attribute list to appear here"
//@ diag: 13:9 error "reference to unresolved using declaration"
//@ diag: 11:1 note "using declaration annotated with 'using_if_exists' here"
//@ diag: 13:16 error "reference to unresolved using declaration"
//@ diag: 12:28 note "using declaration annotated with 'using_if_exists' here"
namespace N { int a; }
using N::nope __attribute__((__using_if_exists__));
[[clang::using_if_exists]] using N::nope2;
int k = nope + nope2;
