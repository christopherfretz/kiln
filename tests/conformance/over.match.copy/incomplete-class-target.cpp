// [over.match.copy], [dcl.init.general]/16.6, [dcl.fct.default]/1: an
// implicit conversion or a copy-initialization to a class type needs the
// class to be complete: "no viable conversion from 'int' to incomplete
// type 'I'", and a default argument of incomplete class type is
// "variable has incomplete type"; explicit conversions and
// `new I(1)` report the incomplete type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:12 error "no matching function for call to 'f'"
//@ diag: 29:6 note "candidate function not viable: no known conversion from 'int' to 'I' for 1st argument"
//@ diag: 32:18 error "'I' is an incomplete type"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 33:18 error "'I' is an incomplete type"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 34:18 error "'I' is an incomplete type"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 35:16 error "allocation of incomplete type 'I'"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 37:27 error "variable has incomplete type 'I'"
//@ diag: 38:17 note "in instantiation of static data member 'Q<I>::t' requested here"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 40:11 error "variable has incomplete type 'I'"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 41:29 error "no viable conversion from 'int' to incomplete type 'I'"
//@ diag: 42:13 note "in instantiation of default function argument expression for 'd2<I>' required here"
//@ diag: 28:8 note "forward declaration of 'I'"
//@ diag: 41:29 note "passing argument to parameter here"
struct I;
void f(I);
void a() { f(1); }
I g();
void b() { (void)(I)1; }
void c() { (void)static_cast<I>(1); }
void d() { (void)I(1); }
void e() { new I(1); }
template<class T> struct Q { static T t; };
template<class T> T Q<T>::t = 1;
template struct Q<I>;
struct A { int x; I *p; };
void d1(I = 1);
template<class T> void d2(T = 1);
void d3() { d2<I>(); }
