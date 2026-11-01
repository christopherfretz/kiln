//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:11 error "reference to 'f' is ambiguous"
//@ diag: 26:6 note "candidate found by name lookup is 'N::f'"
//@ diag: 25:40 note "candidate found by name lookup is 'N::I::f'"
//@ diag: 30:12 error "reference to 'f' is ambiguous"
//@ diag: 25:40 note "candidate found by name lookup is 'N::I::f'"
//@ diag: 26:6 note "candidate found by name lookup is 'N::f'"
//@ diag: 32:23 error "reference to 'v' is ambiguous"
//@ diag: 25:47 note "candidate found by name lookup is 'N::I::v'"
//@ diag: 26:15 note "candidate found by name lookup is 'N::v'"
//@ diag: 33:15 error "call to 'h' is ambiguous"
//@ diag: 25:55 note "candidate function"
//@ diag: 26:23 note "candidate function"
//@ diag: 36:18 error "reference to 'z' is ambiguous"
//@ diag: 35:5 note "candidate found by name lookup is 'P::z'"
//@ diag: 34:40 note "candidate found by name lookup is 'P::I::z'"
//@ diag: 34:70 note "candidate found by name lookup is 'P::J::z'"
// [basic.scope.scope]/4, [namespace.def.general]/6: an inline namespace's
// member and a same-named declaration in the enclosing namespace are
// distinct entities; unqualified and qualified lookup in the enclosing
// namespace find both, and a use is ambiguous ([basic.lookup.general]/1).
// clang lists the names declared in the open namespace definition first
// (most recent first), then the namespace's lookup table.
namespace N { inline namespace I { int f; int v; void h(int); }
void f(); int v; void h(int) {}
int g() { f(); return 0; }
}
namespace N {
int g2() { f(); return 0; }
}
int use() { return N::v; }
void call() { N::h(1); }
namespace P { inline namespace I { int z; } inline namespace J { int z; }
int z;
int w() { return z; }
}
