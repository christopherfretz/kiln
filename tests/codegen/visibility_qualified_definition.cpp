//@ args: -std=c++26
// Visibility of a qualified definition at file scope (spec/codegen.md §8
// "Namespace scope"; clang getLVForNamespaceScopeDecl from
// getDeclContext(); WP-2725): the definition belongs to the namespace
// definition the qualifier's lookup found, the latest one before it, so
// it takes that definition's attribute, not the first's (a, b, c, g, and
// the variable h)
// nor the one its earlier declaration is in (b, d); a nested qualifier
// names the latest definitions of each (e). An unqualified definition in
// a plain definition is default whatever its earlier declaration's
// namespace definition says (f).
namespace __attribute__((visibility("hidden"))) a { int f(); }
namespace a {}
int a::f() { return 0; }
namespace b { int f(); }
namespace __attribute__((visibility("hidden"))) b {}
int b::f() { return 0; }
namespace c {}
namespace __attribute__((visibility("hidden"))) c { int f(); }
int c::f() { return 0; }
namespace d { int f(); }
namespace __attribute__((visibility("hidden"))) d { int f(); }
namespace d {}
int d::f() { return 0; }
namespace __attribute__((visibility("hidden"))) e { namespace n { int f(); } }
namespace e { namespace n {} }
int e::n::f() { return 0; }
namespace f { int f(); }
namespace __attribute__((visibility("hidden"))) f { int f(); }
namespace f { int f() { return 0; } }
namespace g { template <class T> int t() { return 0; } }
namespace __attribute__((visibility("hidden"))) g {}
template <> int g::t<int>() { return 1; }
namespace h { extern int v; }
namespace __attribute__((visibility("hidden"))) h {}
int h::v = 1;
int use() { return g::t<int>() + h::v; }
