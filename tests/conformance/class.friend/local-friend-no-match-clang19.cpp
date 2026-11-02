// [class.friend]/6, [namespace.udecl]/3: clang 19's wording for a local
// class friend definition with nothing to redeclare, and for a member
// using-declaration naming its own class (also "no member named"). kiln
// keeps it at -fclang-compat=19; the clang 22 wording is tested by
// local-friend-definition.cpp and namespace.udecl/class-scope.cpp (WP-377).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:17 error "no matching function found in local scope"
//@ diag: 18:17 error "no matching function 'outer' found in local scope; did you mean '::outer'?"
//@ diag: 14:6 note "'::outer' declared here"
//@ diag: 21:18 error "using declaration refers to its own class"
//@ diag: 21:21 error "no member named 'q' in 'F'"
//@ requires: clang<22
void outer();
void f() {
  struct L {
    friend void q() {}
    friend void outer();
  };
}
struct F { using F::q; };
