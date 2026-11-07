// [over.ics.list]/3, [dcl.init.list]/3.10: a braced argument whose only
// element is reference-related to a reference parameter's referee is that
// binding, also when it fails: the note names the binding failure, not the
// list (clang `TryListConversion`). A non-aggregate-related element keeps
// the list note. WP-724.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 error "no matching function for call to 'p'"
//@ diag: 15:5 note "candidate function not viable: expects an rvalue for 1st argument"
//@ diag: 23:3 error "no matching function for call to 't'"
//@ diag: 16:5 note "candidate function not viable: 1st argument ('const O') would lose const qualifier"
//@ diag: 24:3 error "no matching function for call to 'w'"
//@ diag: 18:5 note "candidate function not viable: cannot convert initializer list argument to 'const E'"
struct O { bool a = false; unsigned long long b = 0; };
int p(O &&);
int t(O &);
struct E { int a; };
int w(const E &);
void k() {
  O x;
  const O cx;
  p({x});
  t({cx});
  w({{x}});
}
