// [basic.scope.hiding]/2: a data member hides a class or enumeration of
// the same name declared in the same class, which an elaborated-type-
// specifier still names (a vendored header's `struct generic_request {...}
// generic_request;`).
//@ strict-diags
struct S {
  struct g { int a; } g;
  enum e { E0 } e;
  struct h { int b; };
  static struct h h;
};
struct S::h S::h = {2};
int f(S s) { struct S::g x = s.g; enum S::e y = s.e; return x.a + y + S::h.b; }
