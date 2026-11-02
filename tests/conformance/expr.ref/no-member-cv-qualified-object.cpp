// [expr.ref]/7.4, [class.member.lookup]/1: member lookup in the class of a
// cv-qualified object expression that finds nothing names the class
// without the object's cv-qualifiers ("no member named 'missing' in 'It'"
// for a `const It&`, a `const It*` and a `const volatile It&`), also in an
// instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:5 error "no member named 'missing' in 'It'"
//@ diag: 18:6 error "no member named 'missing' in 'It'"
//@ diag: 19:5 error "no member named 'missing' in 'It'"
//@ diag: 22:5 error "no member named 'missing' in 'It'"
//@ diag: 25:15 note "in instantiation of function template specialization 'g<It>' requested here"
//@ diag: 23:6 error "no member named 'missing' in 'It'"

struct It { int x; };
void f(const It& o, const It* p, const volatile It& v) {
  o.missing();
  p->missing();
  v.missing();
}
template <class T> void g(const T& o, const T* p) {
  o.missing();
  p->missing();
}
template void g<It>(const It&, const It*);
