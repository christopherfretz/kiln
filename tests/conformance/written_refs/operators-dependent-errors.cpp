// Dependent operators whose instantiation fails (spec/ast.md §18.13.6): an
// operator that selects nothing in an ill-formed specialization
// ([temp.res.general]/6; [over.match.oper]/3 finds no viable function, or
// `->` applied to a class with no `operator->`, [over.ref]/1) has an
// `(unresolved)` row; the specialization that succeeds has its row.
//@ expect: compile-fail
//@ written-refs: operators-dependent-errors.refs
//@ diag: 16:44 error "invalid operands to binary expression ('S' and 'S')"
//@ diag: 17:45 error "member reference type 'S' is not a pointer; did you mean to use '.'?"
struct S {
  int v;
};
struct P {
  S *operator->();
};
template <class T> int mul(T t) { return t * t; }
template <class T> int arrow(T t) { return t->v; }

int run() { return mul(S{}) + mul(2) + arrow(S{}) + arrow(P{}); }
