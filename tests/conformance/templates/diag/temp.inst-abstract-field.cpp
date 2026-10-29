// [class.abstract]/5: a field of a substituted abstract class type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:33 error "field type 'A' is an abstract class"
//@ diag: 9:6 note "in instantiation of template class 'H<A>' requested here"
//@ diag: 7:25 note "unimplemented pure virtual method 'f' in 'A'"
struct A { virtual void f() = 0; };
template <class T> struct H { T t; };
H<A> h;
