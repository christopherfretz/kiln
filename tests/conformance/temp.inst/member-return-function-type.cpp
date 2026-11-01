// [dcl.fct]/11, [temp.inst]/3: instantiating a class template's member
// declaration whose return type becomes a function or array type is
// ill-formed, diagnosed at the return type as written (clang's return
// TypeLoc begin: `T`, `auto` of a trailing return, not `const`/`static`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:31 error "function cannot return function type 'int ()'"
//@ diag: 19:10 note "in instantiation of template class 'C<int ()>' requested here"
//@ diag: 20:49 error "function cannot return function type 'int ()'"
//@ diag: 21:11 note "in instantiation of template class 'C2<int ()>' requested here"
//@ diag: 22:32 error "function cannot return function type 'int ()'"
//@ diag: 23:11 note "in instantiation of template class 'C3<int ()>' requested here"
//@ diag: 24:38 error "function cannot return function type 'int ()'"
//@ diag: 25:11 note "in instantiation of template class 'C5<int ()>' requested here"
//@ diag: 26:32 error "function cannot return array type 'int[2]'"
//@ diag: 27:12 note "in instantiation of template class 'C7<int[2]>' requested here"
//@ diag: 29:15 error "function cannot return function type 'F' (aka 'int ()')"
template <class T> struct C { T mm(); };
C<int()> c;
template <class T> struct C2 { constexpr static T mm(); };
C2<int()> c2;
template <class T> struct C3 { auto mm() -> T; };
C3<int()> c3;
template <class T> struct C5 { const T mm(); };
C5<int()> c5;
template <class T> struct C7 { T operator()(); };
C7<int[2]> c7;
typedef int F();
struct D { F g(); };
