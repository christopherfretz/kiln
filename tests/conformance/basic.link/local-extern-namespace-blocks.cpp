// [basic.link]/6, [namespace.def.general]/2: a block-scope declaration redeclares a later declaration in any block of its namespace, not one of an inline namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:20 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 14:37 note "previous declaration is here"
//@ diag: 17:20 error "redefinition of 'v' with a different type: 'long' vs 'int'"
//@ diag: 16:43 note "previous declaration is here"
//@ diag: 19:18 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 18:35 note "previous declaration is here"
// The same entity in another block: linked, so an exact redeclaration is
// valid. An inline namespace is a namespace of its own: neither direction
// links (clang).
namespace s { inline int f() { extern int w; int h(int); return w + h(1); } }
namespace s { inline void t() { int k(int); } }
namespace s { void k(int); }
namespace s { inline int u() { extern int v; return v; } }
namespace s { long v; }
namespace { inline void a() { int m(char); } }
namespace { void m(char); }
namespace s { int w; int h(int); }
namespace q { inline namespace I { inline void t() { int k(int); } } }
namespace q { void k(int); }
namespace q { inline void t2() { int m(int); } inline namespace J { void m(int); } }
