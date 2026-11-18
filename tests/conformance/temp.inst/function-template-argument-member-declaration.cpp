// [temp.inst]/3, /11, [basic.def.odr]/7: a function template
// specialization named as a template argument in a class template's member
// declaration (outside a member function body) is not instantiated, in the
// template's definition or in an instantiation of the class: kiln keeps
// clang 22's behaviour (a recorded decision, docs/status/deviations.md
// "Template argument in a member declaration"). GCC 16.1 instantiates
// `f<int>` and `f<long>` when the definition completes `A<&f<int>>` and
// `A<&f<long>>`, and rejects this (not `f<short>`: `A<&f<short>>*`
// completes nothing).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
template <class T> void f() { typename T::nope x; }
template <void (*)()> struct A {};
template <class T> struct B { A<&f<int>> m; };
B<char> b;
template <class T> struct C { static A<&f<long>> sm; A<&f<short>>* p; };
C<char> c;
int main() {}
