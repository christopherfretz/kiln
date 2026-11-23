//@ args: -std=c++26
// Visibility of a declaration (an undefined reference; spec/codegen.md §8
// "Where it applies"; clang GetOrCreateLLVMFunction(GD),
// GetOrCreateLLVMGlobal; WP-2726): the global takes the visibility of the
// declaration the use names, the most recent one visible there, not the
// first's: hidden when that one is in a hidden namespace definition (a,
// v), default when a later plain one follows (b, c). A constant address
// takes the most recent declaration's when it is emitted (p). The global
// keeps what its first use gave it: a use emitted before the hidden
// redeclaration leaves it default (d, as clang). A using-declaration in a
// hidden definition does not change the function it names (u).
namespace a { int f(); }
namespace __attribute__((visibility("hidden"))) a { int f(); }
int ua() { return a::f(); }
namespace n { extern int v; }
namespace __attribute__((visibility("hidden"))) n { extern int v; }
int uv() { return n::v; }
namespace __attribute__((visibility("hidden"))) b { int f(); }
namespace b { int f(); }
int ub() { return b::f(); }
namespace c { int f(); extern int v; }
namespace __attribute__((visibility("hidden"))) c { int f(); extern int v; }
namespace c { int f(); extern int v; }
int uc() { return c::f() + c::v; }
namespace d { int f(); }
int ud() { return d::f(); }
namespace __attribute__((visibility("hidden"))) d { int f(); }
int ud2() { return d::f(); }
namespace q { int f(); }
namespace __attribute__((visibility("hidden"))) q { int f(); }
int (*p)() = &q::f;
namespace u { void f(); }
namespace __attribute__((visibility("hidden"))) u { using ::u::f; }
void uu() { u::f(); }
