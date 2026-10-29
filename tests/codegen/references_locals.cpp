// Local references bound to non-constant lvalues (a reference with a
// constant initializer is folded to its referee by clang; kiln's Oven does
// not fold reference initializers yet); references to arrays, functions
// and incomplete classes; reference-returning calls used as lvalues.
struct P { char c; double d; };
struct Fwd;
int arr[4] = {1, 2, 3, 4};
int sum3(int (&a)[4]) { return a[0] + a[1] + a[3]; }
int twice(int v) { return 2 * v; }
int apply(int (&f)(int), int v) { return f(v); }
int& id(int& x) { return x; }
double& field(P& p) { return p.d; }
Fwd& same(Fwd& f) { return f; }
int locals(int x, P p) {
  int& r = x;
  int& rr = id(r);
  r += 3;
  field(p) = 2.5;
  id(x) = 9;
  return r + rr + sum3(arr) + apply(twice, x) + (int)p.d;
}
