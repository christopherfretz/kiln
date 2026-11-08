// [expr.prim.lambda.capture]: a lambda local that hides an enclosing
// local warns under -Wshadow when the variable is captured (with a note at
// the explicit capture) and under -Wshadow-uncaptured-local (in
// -Wshadow-all) when it is not. Init-captures and lambda parameters are
// checked too; a hidden field follows whether `this` is captured.
//@ verify
//@ args: -Wshadow-all

void lam(int x, int y) { // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
  auto a = [] { int x = 0; return x; }; // expected-warning {{declaration shadows a local variable}}
  auto b = [x] { int x = 0; return x; }; // expected-warning {{declaration shadows a local variable}} expected-note {{variable 'x' is explicitly captured here}}
  auto c = [=] { int x = 0; return x + y; }; // expected-warning {{declaration shadows a local variable}}
  auto d = [=] { int y = 0; return y + x; }; // expected-warning {{declaration shadows a local variable}}
  auto e = [&] (int y) { return y; }; // expected-warning {{declaration shadows a local variable}}
  auto f = [] (int x) { return x; }; // expected-warning {{declaration shadows a local variable}}
  auto g = [x] (int y) { return x + y; }; // expected-warning {{declaration shadows a local variable}}
  auto h = [z = x] { return z; };
  auto i = [x = x] { return x; }; // expected-warning {{declaration shadows a local variable}}
  auto j = [y] { return [y] { int y = 1; return y; }(); }; // expected-warning {{declaration shadows a local variable}} expected-note {{variable 'y' is explicitly captured here}}
  auto k = [=] { return [=] { int x = 1; return x; }(); }; // expected-warning {{declaration shadows a local variable}}
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)f; (void)g; (void)h;
  (void)i; (void)j; (void)k;
}
void ec() { int c = 0; auto l = [=, &c] { int c = 1; return c; }; (void)l; } // expected-warning {{declaration shadows a local variable}} expected-note {{variable 'c' is captured here}} expected-note {{previous declaration is here}}
struct M {
  int f; int bp; // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
  void m() {
    auto a = [] { int f = 0; return f; }; // expected-warning {{declaration shadows a field of 'M'}}
    auto b = [this] { int f = 0; return f; }; // expected-warning {{declaration shadows a field of 'M'}}
    auto c = [&] { int f = 0; return f + bp; }; // expected-warning {{declaration shadows a field of 'M'}}
    auto d = [=] { int f = 0; return f; }; // expected-warning {{declaration shadows a field of 'M'}}
    (void)a; (void)b; (void)c; (void)d;
  }
};
