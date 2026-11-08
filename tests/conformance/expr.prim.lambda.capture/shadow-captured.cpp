// [expr.prim.lambda.capture]: under plain -Wshadow an uncaptured enclosing
// local does not warn; a captured one does.
//@ verify
//@ args: -Wshadow

void lam(int x) { // expected-note {{previous declaration is here}}
  auto a = [] { int x = 0; return x; };
  auto b = [x] { int x = 0; return x; }; // expected-warning {{declaration shadows a local variable}} expected-note {{variable 'x' is explicitly captured here}}
  auto c = [] (int x) { return x; };
  (void)a; (void)b; (void)c;
}
