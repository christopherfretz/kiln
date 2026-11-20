// [basic.scope.scope]/4: a member function cannot share its name with a
// variable template of its class (clang: "redefinition of 'v' as different
// kind of symbol"). Redeclaration lookup for a non-template function skips
// every template, so kiln accepts the declaration, and a later `u.v(1)`
// becomes an ambiguous reference instead.
//@ xfail(kiln): WP-1738 follow-up, pick_prev_function skips a variable template for a non-template function, so the conflict is not diagnosed
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:8 error "redefinition of 'v' as different kind of symbol"
//@ diag: 14:43 note "previous definition is here"
//@ diag: 17:7 error "duplicate member 'w'"
//@ diag: 16:27 note "previous declaration is here"
struct U {
  template <class X> static constexpr int v = 1;
  void v(int);
  template <class X> void w();
  int w;
};
