//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:1 error "constraints not satisfied for class template 'C' [with T = int]"
//@ diag: 10:11 note "because 'int' does not satisfy 'Huge'"
//@ diag: 9:35 note "because 'sizeof(int) == 1000' (4 == 1000) evaluated to false"
// [temp.constr.decl]/3: the first unsatisfied constraint reported is the
// type-constraint, which comes before the requires-clause even though the
// requires-clause is also unsatisfied.
template <class T> concept Huge = sizeof(T) == 1000;
template <Huge T>
  requires(sizeof(T) == 3)
struct C {};
C<int> c;
