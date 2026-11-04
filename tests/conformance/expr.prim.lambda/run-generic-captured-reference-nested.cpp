// [expr.prim.lambda.capture]/8, /10: a lambda nested in a generic lambda
// that names a reference local of the enclosing function makes both
// lambdas capture it, whether the inner lambda is generic or not and
// whether its full-expression depends on a lambda parameter. The inner
// closure is formed again in each specialization of the outer call
// operator; a variable its body names in a non-dependent expression is
// captured there as a fresh reference would be (WP-581).
//@ expect: run
//@ std: c++20
//@ stdout: 8 8 9 10 10 4
extern "C" int printf(const char*, ...);

static int plain_inner(const int& r) {
  const int& rr = r;
  return [&](auto x) { return [&] { return x + rr; }(); }(3);
}
static int generic_inner(const int& r) {
  const int& rr = r;
  return [&](auto x) { return [&](auto y) { return y + rr; }(x); }(3);
}
static int nondependent_inner(const int& r) {
  const int& rr = r;
  return [&](auto x) { return [&](auto y) { int k = rr; return k + y + 0 * x; }(4); }(1);
}
static int int_local(int r) {
  int v = r;
  return [&](auto x) { return [&](auto y) { return y == 0 ? 0 : v + y; }(x); }(5);
}
static int by_copy(int r) {
  const int& rr = r;
  return [=](auto x) { return [=](auto y) { return y + rr; }(x); }(5);
}
template <class T>
static T in_template(const T& r) {
  const T& rr = r;
  return [&](auto x) { return [&](auto y) { return x + y + rr; }(x); }(T(1));
}

int main() {
  printf("%d %d %d %d %d %d\n", plain_inner(5), generic_inner(5), nondependent_inner(5),
         int_local(5), by_copy(5), in_template(2));
}
