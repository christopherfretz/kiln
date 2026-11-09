// [basic.start.main]/3 valid neighbours of the C-linkage rule (WP-1040): a
// class member named `main` in a class declared in `extern "C"`, a `static`
// (internal, so no language linkage) entity, an `extern "C++"` function in a
// namespace, and a local variable of an `extern "C"` function.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
extern "C" struct A { int main(); static int main2; };
namespace ns { extern "C" struct B { int main; }; }
namespace N { extern "C++" int main(); }
extern "C" { namespace Y { static int main; } }
namespace W { extern "C" { void f() { int main = 0; (void)main; } } }
template<class T> T main;
