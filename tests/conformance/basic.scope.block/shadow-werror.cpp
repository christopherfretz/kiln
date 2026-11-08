// [basic.scope.block]: -Werror=shadow makes the hiding warning an error;
// -Wno-shadow-uncaptured-local keeps that subgroup off under -Wshadow-all.
//@ verify
//@ args: -Wshadow-all -Werror=shadow -Wno-shadow-uncaptured-local

int g; // expected-note {{previous declaration is here}}
void h(int x) {
  int g = 0; (void)g; // expected-error {{declaration shadows a variable in the global namespace}}
  auto l = [] { int x = 2; return x; }; (void)l;
}
