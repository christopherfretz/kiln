// [class.mi]/4, [class.member.lookup]: a direct base that is also an indirect
// base is inaccessible due to ambiguity (clang -Winaccessible-base, on by
// default); naming its members is ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 warning "direct base 'V' is inaccessible due to ambiguity:"
//@ diag: 14:12 warning "direct base 'V' is inaccessible due to ambiguity:"
//@ diag: 17:5 error "non-static member 'v' found in multiple base-class subobjects of type 'V':"
//@ diag: 10:16 note "member found by ambiguous name lookup"
struct V { int v; };
struct N1 : V {};
struct M : virtual V, N1 {};
struct P : N1 {};
struct Q : V, P {};
M m;
void g() {
  m.v = 3;
}
