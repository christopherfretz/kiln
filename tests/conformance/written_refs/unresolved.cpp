// A failed lookup is `UNRESOLVED`, never `DEPENDENT` (spec/ast.md §18.2,
// WP-2291): outside a template an undeclared identifier, a member name
// after an invalid object expression or of a non-class one, and the
// callee of a call no candidate of several fits name nothing (a lone
// candidate is still named); in a template pattern a non-dependent
// name is bound at the definition ([temp.res.general]), so its failed
// lookup is `UNRESOLVED` too, while a dependent name ([temp.dep.general])
// and the callee of a call with a dependent argument
// ([temp.dep.candidate]) stay `DEPENDENT`.
//@ expect: compile-fail
//@ written-refs: unresolved.refs
//@ diag: 27:9 error "use of undeclared identifier 'missing'"
//@ diag: 29:10 error "use of undeclared identifier 'absent'"
//@ diag: 30:4 error "member reference base type 'int' is not a structure or union"
//@ diag: 31:3 error "use of undeclared identifier 'nothing'"
//@ diag: 32:10 error "no matching function for call to 'one'"
//@ diag: 32:21 error "no matching function for call to 'two'"
//@ diag: 36:10 error "use of undeclared identifier 'nodep'"
//@ diag: 37:10 error "use of undeclared identifier 'nocall'"
//@ diag: 38:10 error "no matching function for call to 'one'"
//@ diag: 38:21 error "no matching function for call to 'two'"
//@ diag: 39:3 error "use of undeclared identifier 'noobj'"
struct M {};
int one(int);
int two(int);
int two(long);
int a = missing + 1;
void f(int p) {
  (void)(absent);
  p.mem;
  nothing.mem;
  (void)(one(M{}) + two(M{}));
}
template <class T> void g(T t) {
  (void)(dep(t) + other(1, t) + T::value + t.m);
  (void)(nodep);
  (void)(nocall(1));
  (void)(one(M{}) + two(M{}));
  noobj.mem;
  (void)(one(1));
}
