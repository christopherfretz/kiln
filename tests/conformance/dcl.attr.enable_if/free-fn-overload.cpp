// clang's `enable_if` attribute (clang AttrDocs "enable_if", `Sema::IsOverload`;
// [basic.scope.scope]/4, [over.match.best] as clang extends it): free
// functions that differ only in their `enable_if` conditions are overloads,
// each defined once; a call picks the one whose conditions hold, and of two
// viable ones the one whose conditions extend the other's.
//@ expect: run
//@ strict-diags
int f(int n) { return n; }
int f(int n) __attribute__((enable_if(n == 7, "seven"))) { return 100; }
int f(int n) __attribute__((enable_if(n == 7, "seven"), enable_if(n > 0, "pos"))) {
  return 1000;
}
// A declaration followed by its definition with the same conditions is one
// function.
int g(int n) __attribute__((enable_if(n < 0, "neg")));
int g(int n);
int g(int n) __attribute__((enable_if(n < 0, "neg"))) { return -n; }
int g(int n) { return n * 2; }
int main() {
  int v = 3;
  // `v` is not a constant: every condition fails, the plain one is left.
  if (f(v) != 3 || f(7) != 1000 || f(5) != 5) return 1;
  if (g(-4) != 4 || g(4) != 8) return 2;
  return 0;
}
