// [basic.scope.block]: the -Wshadow family is off by default, and -Wall
// -Wextra do not enable it.
//@ verify
//@ args: -Wall -Wextra

// expected-no-diagnostics
int g;
struct A { int a; };
struct S : A { int f; int a; S(int f) { f = 1; (void)f; } };
void h(int x) {
  (void)x;
  int g = 0; (void)g;
  { int x = 1; (void)x; }
  auto l = [] { int x = 2; return x; }; (void)l;
}
