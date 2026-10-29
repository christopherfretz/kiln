// [temp.inst]/2, [basic.def.odr]/12: a context that needs a complete type
// implicitly instantiates the specialization; a template that is declared
// but not defined is diagnosed once per use, and the use's own
// incomplete-type error is suppressed (clang `RequireCompleteType`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:10 error "implicit instantiation of undefined template 'Fwd<int>'"
//@ diag: 17:27 note "template is declared here"
//@ diag: 19:9 error "implicit instantiation of undefined template 'Fwd<char>'"
//@ diag: 17:27 note "template is declared here"
//@ diag: 20:12 error "implicit instantiation of undefined template 'Fwd<long>'"
//@ diag: 17:27 note "template is declared here"
//@ diag: 21:21 error "implicit instantiation of undefined template 'Fwd<short>'"
//@ diag: 17:27 note "template is declared here"
//@ diag: 24:11 error "implicit instantiation of undefined template 'Fwd<char>'"
//@ diag: 17:27 note "template is declared here"
template <class T> struct Fwd;
Fwd<int> f;
int s = sizeof(Fwd<char>);
struct D : Fwd<long> {};
Fwd<short> *p = new Fwd<short>;
int g() {
  Fwd<char> *q = nullptr;
  return q->x;
}
