// [expr.prim.lambda.closure]/5, [dcl.fct]/6 (P0847): a lambda whose
// call operator has an explicit object parameter reads its captures
// through that parameter, whether it is a copy (`this auto`), a const
// reference, or a forwarding reference, and can recurse through it.
//@ expect: run
//@ std: c++23
//@ strict-diags
struct B {
  int m;
  int get() {
    auto r = [this](this auto const& s) { return m; };
    return r();
  }
};
int main() {
  int a = 5;
  auto by_val = [a](this auto self, int k) { return a + k + a; };
  if (by_val(1) != 11) return 1;
  auto by_ref = [a](this auto const& self, int k) { return a * k; };
  if (by_ref(3) != 15) return 2;
  auto fwd = [x = 7](this auto&& self) { return x; };
  if (fwd() != 7) return 3;
  auto rec = [a](this auto self, int n) -> int {
    return n == 0 ? a : self(n - 1) + 1;
  };
  if (rec(3) != 8) return 4;
  int r = 0;
  auto refcap = [&r](this auto const& self, int v) { r += v; r += v; };
  refcap(2);
  if (r != 4) return 5;
  B b{9};
  if (b.get() != 9) return 6;
  return 0;
}
