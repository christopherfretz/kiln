// [temp.dep.candidate]/1, [over.match.oper]/3: an operator declared after
// a template's definition in a namespace that is not associated with the
// operands is not a candidate at instantiation: the dependent operator
// expression has no viable function. Not strict: kiln reports "invalid
// operands" where clang names the operator that is neither visible at
// the definition nor found by argument-dependent lookup (WP-1510:
// clang's two-phase operator lookup diagnostic).
//@ expect: compile-fail
namespace N { struct S {}; }
template<class X> int h(X x) { return x - 1; }
template<class X> int c(X x) { return ~x; }
int operator-(N::S, int) { return 2; }
int operator~(N::S) { return 3; }
int b = h(N::S{}) + c(N::S{});
