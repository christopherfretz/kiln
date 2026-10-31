// [temp.inst], [stmt.ranged]: a dependent range is checked when the
// template is instantiated, with the instantiation note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:14 error "invalid range expression of type 'const E'; no viable 'begin' function available"
//@ diag: 13:18 note "in instantiation of function template specialization 'sum<E>' requested here"
template <class C> int sum(const C& c) {
  int s = 0;
  for (int x : c) s += x;
  return s;
}
struct E {};
int g() { return sum(E{}); }
