// Function-name spans (spec/ast.md §24.4): each printed namespace is a
// `scope` entry naming that namespace (its first declaration, so the
// reopened `a` is one entity); an inline namespace is not printed; the
// anonymous namespace is one synthesized entry.
//@ fn-names: namespaces.names
//@ std: c++20
namespace a { void f() { (void)__PRETTY_FUNCTION__; } }
namespace a::b { void g(int) { (void)__PRETTY_FUNCTION__; } }
namespace a { inline namespace v1 { void h() { (void)__PRETTY_FUNCTION__; } } }
namespace a { namespace { struct A {}; void anon(A) { (void)__PRETTY_FUNCTION__; } } }
void top() { (void)__PRETTY_FUNCTION__; }
void use() { a::f(); a::b::g(1); a::h(); a::anon({}); top(); }
