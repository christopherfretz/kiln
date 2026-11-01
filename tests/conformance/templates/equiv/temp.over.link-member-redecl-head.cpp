//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:66 error "constructor cannot be redeclared"
//@ diag: 12:66 note "previous declaration is here"
//@ diag: 16:48 error "constructor cannot be redeclared"
//@ diag: 15:48 note "previous declaration is here"
// [temp.over.link]/6-7: member templates whose heads differ only in
// parameter names or default arguments, with equivalent requires-clauses,
// redeclare the same member ([class.mem.general]/5).
template <bool B, class T = void> struct EI {};
template <class A> struct S {
  template <class X, typename EI<sizeof(X) == 4, int>::type = 0> S(X);
  template <class Y, typename EI<sizeof(Y) == 4, int>::type = 1> S(Y);
  template <class X> S(X, int);
  template <class X> requires (sizeof(X) == 4) S(X, int);
  template <class X> requires (sizeof(X) == 4) S(X, int);
};
