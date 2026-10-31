//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:17 error "no matching member function for call to 'f'"
//@ diag: 8:53 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 8:32 note "because 'sizeof(int) == 2' (4 == 2) evaluated to false"
// [over.match.viable]/3: an unsatisfied constrained member function template.
struct S {
  template <class T> requires (sizeof(T) == 2) void f(T);
};
void g(S s) { s.f(1); }
