// [temp.friend]/9, [temp.constr.decl]/3 (WP-231): the enclosing
// specialization's argument makes the friend's constraint unsatisfied.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:9 error "no matching function for call to 'f'"
//@ diag: 14:33 note "candidate template ignored: constraints not satisfied [with U = int]"
//@ diag: 14:50 note "because 'char' does not satisfy 'C'"
//@ diag: 12:32 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
//@ diag: 18:9 error "no matching function for call to 'g'"
//@ diag: 15:59 note "candidate template ignored: constraints not satisfied [with U = long]"
//@ diag: 15:32 note "because 'sizeof(int) == 8' (4 == 8) evaluated to false"
template <class T> concept C = sizeof(T) == 4;
template <class T> struct X {
  template <class U> friend int f(X, U) requires C<T> { return 1; }
  template <class U> requires (sizeof(T) == 8) friend int g(X, U) { return 2; }
};
int a = f(X<char>{}, 1);
int b = g(X<int>{}, 1L);
