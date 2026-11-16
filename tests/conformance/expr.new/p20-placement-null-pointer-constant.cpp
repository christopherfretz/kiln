// [expr.new]/20, [conv.ptr]/1: the placement arguments of a new-expression
// are the arguments of an ordinary overload resolution among the
// allocation functions, so a null pointer constant converts to the
// `void *` of the reserved placement form, a class's own `operator new
// (size_t, void *)` or an array form, and an exact match against an `int`
// parameter beats that conversion ([over.ics.rank]/4). Found running GCC's
// C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++17
#include <new>
struct X { int a; };
struct Y {
  void *operator new(std::size_t, void *p) { return p; }
  int b;
};
struct Tag {};
void *operator new(std::size_t, Tag, int n);
X *one() { return new (0) X; }
X *null() { return new (nullptr) X; }
X *many() { return new (0) X[3]; }
Y *own() { return new (0) Y; }
X *zero_long() { return new (0L) X; }
// `int` is an exact match for `0`; the reserved form is not viable for
// the `Tag` first argument.
X *exact(Tag t) { return new (t, 0) X; }
template <class T> T *dependent() { return new (0) T; }
template X *dependent<X>();
