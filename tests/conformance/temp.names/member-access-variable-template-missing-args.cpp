// [temp.names]/1: a static data member template named by a class member
// access needs its template arguments (clang's "use of variable template
// requires template arguments"; WP-789).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:27 error "use of variable template 'twice' requires template arguments"
//@ diag: 9:41 note "template is declared here"
struct cpo_t {
  template <int N> static constexpr int twice = 2 * N;
};
int f(cpo_t c) { return c.twice; }
