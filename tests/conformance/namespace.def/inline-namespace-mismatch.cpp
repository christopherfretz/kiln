// [namespace.def.general]/3: an extension definition of a non-inline
// namespace shall not be `inline`; clang errors with a note at the
// original definition (`DiagnoseNamespaceInlineMismatch`) and recovers
// with the namespace non-inline (so `B::d` is still found through `B`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:18 error "non-inline namespace cannot be reopened as inline"
//@ diag: 9:11 note "previous definition is here"
namespace B { int c; }
inline namespace B { int d; }
int x = B::c + B::d;
