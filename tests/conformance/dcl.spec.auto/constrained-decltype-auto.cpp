// [dcl.spec.auto.general]/1, [dcl.type.auto.deduct]/2: a block-scope
// declaration may start with a constrained `decltype(auto)` placeholder
// (`C<A> decltype(auto) x = e;`); the deduced type is the first argument
// of the type-constraint (libc++'s array/iterators.pass.cpp). An
// instantiation's diagnostic points at the pattern's type-constraint.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:3 error "deduced type 'int &' does not satisfy 'same_as<int>'"
//@ diag: 29:15 note "in instantiation of function template specialization 'h<int>'"
//@ diag: 14:47 note "because '__is_same(int &, int)' evaluated to false"
//@ diag: 30:20 error "deduced type 'int' does not satisfy 'same_as<long>'"
//@ diag: 31:9 note "in instantiation of function template specialization 'k<int>'"
//@ diag: 14:47 note "because '__is_same(int, long)' evaluated to false"
template <class T, class U> concept same_as = __is_same(T, U);
int x = 0;
int& f() { return x; }
void g() {
  same_as<int&> decltype(auto) r = f();
  same_as<int> decltype(auto) v = 3;
  for (same_as<int> decltype(auto) i = 0; i < 1; ++i) {}
  if (same_as<int&> decltype(auto) c = f()) {}
  (void)r;
  (void)v;
}
template <class T> void h(T t) {
  same_as<int> decltype(auto) a = (t);
  (void)a;
}
template void h(int);
template <class T> same_as<long> auto k(T t) { return t; }
int z = k(1);
