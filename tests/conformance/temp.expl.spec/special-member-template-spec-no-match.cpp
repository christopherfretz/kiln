// [temp.expl.spec]/1, [temp.deduct.decl]: an out-of-line constructor
// declarator after `template<>` that matches neither a non-template
// constructor nor, by deduction, a constructor template is "no function
// template matches function template specialization", with each
// constructor template noted as a candidate that failed deduction.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:16 error "no function template matches function template specialization 'P'"
//@ diag: 15:22 note "candidate template ignored: failed template argument deduction"
//@ diag: 18:16 error "no function template matches function template specialization 'P'"
//@ diag: 15:22 note "candidate template ignored: could not match 'const T &' against 'double'"
struct J { int v; };
struct P {
  P(int) {}
  template <class T> P(const T&) {}
};
template <> P::P(const J&, int) {}
template <> P::P(double) {}
