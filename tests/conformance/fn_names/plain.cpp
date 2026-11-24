// Function-name spans (spec/ast.md §24.2): `__func__`, `__FUNCTION__` and
// `__builtin_FUNCTION()` print the bare name; a string made outside a
// function (`""` here) has no record.
//@ fn-names: plain.names
//@ std: c++20
namespace ns {
struct S { void m() { (void)__func__; } };
void f() { (void)__func__; (void)__FUNCTION__; (void)__builtin_FUNCTION(); }
template <class T> void t(T) { (void)__func__; }
}
const char *outside = __builtin_FUNCTION();
void use() { ns::S{}.m(); ns::f(); ns::t(1); }
