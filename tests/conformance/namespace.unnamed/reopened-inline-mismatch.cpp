// [namespace.unnamed]/1: every unnamed namespace of a scope is the same
// namespace, also in a reopened namespace and inside `extern "C++"`.
// [namespace.def.general]/3 then applies to it as to a named one: clang
// diagnoses the mismatch at the `namespace` keyword and notes the
// original definition at its `{`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:1 warning "inline namespace reopened as a non-inline namespace"
//@ diag: 18:18 note "previous definition is here"
//@ diag: 20:16 warning "inline namespace reopened as a non-inline namespace"
//@ diag: 18:18 note "previous definition is here"
//@ diag: 22:22 error "non-inline namespace cannot be reopened as inline"
//@ diag: 21:25 note "previous definition is here"
//@ diag: 24:15 warning "inline namespace reopened as a non-inline namespace"
//@ diag: 23:32 note "previous definition is here"
//@ diag: 24:31 error "redefinition of 'y'"
//@ diag: 23:38 note "previous definition is here"
inline namespace { int a; }
namespace { int b; }
extern "C++" { namespace { int c; } }
namespace N { namespace { int x; } }
namespace N { inline namespace { int z; } }
namespace O { inline namespace { int y; } }
namespace O { namespace { int y; } }
int g = a + b + c + N::x + N::z;
