// [expr.static.cast]/4, [expr.cast]/4, [expr.reinterpret.cast]/1,
// [expr.const.cast]/1: a static_cast or C-style cast to a class type
// initializes a prvalue of that type, so the class must be complete
// ("... to incomplete type"); reinterpret_cast and const_cast cannot
// produce a class prvalue at all.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:9 error "reinterpret_cast from 'S' to 'S' is not allowed"
//@ diag: 27:9 error "const_cast to 'S', which is not a reference, pointer-to-object, or pointer-to-data-member"
//@ diag: 28:9 error "reinterpret_cast from 'int' to 'T' is not allowed"
//@ diag: 29:9 error "const_cast to 'T', which is not a reference, pointer-to-object, or pointer-to-data-member"
//@ diag: 30:9 error "'I' is an incomplete type"
//@ diag: 23:8 note "forward declaration of 'I'"
//@ diag: 31:9 error "'I' is an incomplete type"
//@ diag: 23:8 note "forward declaration of 'I'"
//@ diag: 32:9 error "'I' is an incomplete type"
//@ diag: 23:8 note "forward declaration of 'I'"
//@ diag: 35:18 error "implicit instantiation of undefined template 'U<int>'"
//@ diag: 34:26 note "template is declared here"
//@ diag: 35:48 error "implicit instantiation of undefined template 'U<int>'"
//@ diag: 34:26 note "template is declared here"
struct S { int x; };
struct I;
struct T { T(int); };
void a(S s, const S cs) {
  (void)reinterpret_cast<S>(s);
  (void)const_cast<S>(cs);
  (void)reinterpret_cast<T>(1);
  (void)const_cast<T>(1);
  (void)static_cast<I>(1);
  (void)(I)1;
  (void)I(1);
}
template<class T> struct U;
void b() { (void)static_cast<U<int>>(1); (void)(U<int>)1; }
