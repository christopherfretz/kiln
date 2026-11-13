// [basic.scope.scope]/4, [over.load], [dcl.array] (the variable length
// array extension in C++): two declarations of a function whose parameter
// has a variably modified type do not correspond, the parameter types
// being distinct, so they declare two overloads: two definitions are no
// redefinition, nor a friend declaration and a definition, and a call
// finds both (as clang 19 and 22). A pointer to the argument's array type
// converts to neither. WP-1405.
//@ expect: compile-fail
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 22:3 error "no matching function for call to 'k'"
//@ diag: 18:6 note "candidate function not viable: no known conversion from 'int (*)[n]' to 'int (*)[n]' for 2nd argument"
//@ diag: 19:6 note "candidate function not viable: no known conversion from 'int (*)[n]' to 'int (*)[n]' for 2nd argument"
void h(int n, int (*p)[n]) {}
void h(int n, int (*p)[n]) {}
struct T { friend void h2(int n, int (*p)[n]); };
void h2(int n, int (*p)[n]) {}
void k(int n, int (*p)[n]);
void k(int n, int (*p)[n]);
void use(int n) {
  int (*p)[n] = nullptr;
  k(n, p);
}
