// [class.qual]/2: in `X<T>::X(...)` the name after the nested-name-specifier
// names the constructor. When a parameter's type is an unknown name the
// declaration is invalid, and the unknown type name is the one error. kiln
// also reported "a type specifier is required" for the constructor name
// (found running GCC's C++ testsuite), as GCC reports one error. A valid
// out-of-line constructor next to it stays accepted.
//@ expect: compile-fail
//@ skip(clang): clang takes `X<T>::X` before an unknown name for a type (implicit typename) and cascades
//@ strict-diags
//@ diag: 17:61 error "unknown type name 'nosuch'"
//@ diag: 19:28 error "unknown type name 'nosuch'"
//@ diag: 21:11 error "unknown type name 'nosuch'"

template <class> struct B { typedef int bar; };
template <template <class> class C, class T>
struct foo { typedef typename C<T>::bar bar; foo(bar); foo(); };
template <template <class> class C, class T> foo<C, T>::foo(nosuch) {}
template <class T> struct X { X(int); X(); };
template <class T> X<T>::X(nosuch) {}
struct Z { Z(int); };
Z::Z(int, nosuch) {}
template <class T> X<T>::X(int) {}
template <template <class> class C, class T> foo<C, T>::foo() {}
X<int> x(1);
foo<B, int> f;
