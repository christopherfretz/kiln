// [basic.scope.scope]/4, [basic.lookup.general]/4, [expr.ref]: a member
// function or typedef-name redeclaring a data member, static data member
// or member function of its class as a different kind of entity is
// invalid, and a member access naming it denotes the valid declaration
// (clang `LookupResult::resolveKind`). Valid declarations that are not
// all member functions (an unscoped enumerator after member functions)
// make the access ambiguous. Before WP-1738 kiln reported "this kind of
// overloaded member access is not supported yet".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:24 error "redefinition of 'm' as different kind of symbol"
//@ diag: 26:16 note "previous definition is here"
//@ diag: 26:37 error "redefinition of 'm' as different kind of symbol"
//@ diag: 26:16 note "previous definition is here"
//@ diag: 27:31 error "redefinition of 's' as different kind of symbol"
//@ diag: 27:23 note "previous definition is here"
//@ diag: 28:53 error "redefinition of 'g' as different kind of symbol"
//@ diag: 28:30 note "previous definition is here"
//@ diag: 31:6 error "called object type 'int' is not a function or function pointer"
//@ diag: 32:6 error "called object type 'int' is not a function or function pointer"
//@ diag: 34:5 error "reference to 'e' is ambiguous"
//@ diag: 29:16 note "candidate found by name lookup is 'E::e'"
//@ diag: 29:28 note "candidate found by name lookup is 'E::e'"
//@ diag: 29:44 note "candidate found by name lookup is 'E::e'"
//@ diag: 35:14 error "invalid operands to binary expression ('int' and 'void')"
struct S { int m; void m(int); void m(double); };
struct U { static int s; void s(int); };
struct T { void g(int); void g(double); typedef int g; };
struct E { int e(int); int e(long); enum { e }; };
int use(S s, U u, T t, E e) {
  s.m(1);
  u.s(1);
  t.g(1);
  e.e(1);
  return s.m + t.g(2.0);
}
