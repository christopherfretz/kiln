// [class.base.init]/2, [temp.res.general]/5: a qualified mem-initializer-id
// with a dependent qualifier that names no type in the specialization is
// ill-formed when the constructor is instantiated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:50 error "no type named 'Foo' in 'D7<int>'"
//@ diag: 12:9 note "in instantiation of member function 'D7<int>::D7' requested here"
//@ diag: 14:46 error "no type named 'Nope' in 'Q'"
//@ diag: 15:7 note "in instantiation of member function 'D4<Q>::D4' requested here"
template <class T> struct B { B(int) {} };
template <class T> struct D7 : B<T> { D7() : D7::Foo(1) {} };
D7<int> d7;
struct Q { };
template <class T> struct D4 : T { D4() : T::Nope(1) {} };
D4<Q> d4;
