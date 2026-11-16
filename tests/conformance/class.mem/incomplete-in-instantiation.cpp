// [class.mem.general]/8, [temp.inst]/3: a class is incomplete until its
// closing brace, also while a specialization its definition needs is
// instantiated, so a field of the class's type there is an error;
// [dcl.init.general]/16.4: an empty pack expansion as a variable's
// parenthesized initializer value-initializes, which an array of unknown
// bound cannot be. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:33 error "field has incomplete type 'W'"
//@ diag: 21:17 note "in instantiation of template class 'Y<W>' requested here"
//@ diag: 21:8 note "definition of 'W' is not complete until the closing '}'"
//@ diag: 22:33 error "field has incomplete type 'Z<int>'"
//@ diag: 23:36 note "in instantiation of template class 'X<Z<int>>' requested here"
//@ diag: 24:40 note "in instantiation of template class 'P<Z<int>>' requested here"
//@ diag: 25:8 note "in instantiation of template class 'Z<int>' requested here"
//@ diag: 24:27 note "definition of 'Z<int>' is not complete until the closing '}'"
//@ diag: 27:46 error "variable has incomplete type 'int[]'"
//@ diag: 28:14 note "in instantiation of function template specialization 'tf<int[]>' requested here"
template <class T> struct Y { T t; };
struct W { Y<W> y; };
template <class T> struct X { T t; };
template <class T> struct P { X<T> x; };
template <class T> struct Z { P<Z<T> > p; };
Z<int> z;
template<class T> T&& mk();
template<class T, class... As> void tf() { T t(mk<As>()...); (void)t; }
void use() { tf<int[]>(); }
