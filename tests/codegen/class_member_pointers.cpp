// Pointers to members (Itanium; the ARM variant on aarch64): data member
// offsets with the -1 null, member function pairs, base-to-derived and
// derived-to-base conversions, .* and ->*, comparisons.
struct B { int a, b; int add(int k) const noexcept { return a + k; } };
struct M { long pad; };
struct D : M, B { int c; int mul(int k) noexcept { return c * k; } };

int B::*data_a() { return &B::a; }
int D::*data_conv(int B::*p) { return p; }
int B::*data_back(int D::*p) { return static_cast<int B::*>(p); }
int (B::*fn_add())(int) const { return &B::add; }
int (D::*fn_conv(int (B::*p)(int) const))(int) const { return p; }

int use(D& d, D* pd, int B::*pb, int (D::*pf)(int)) {
  int D::*pc = &D::c;
  int D::*pa = &B::a;
  int D::*n = nullptr;
  int (D::*pm)(int) = &D::mul;
  int r = d.*pb + pd->*pb + d.*pc + pd->*pa;
  d.*pc = 4;
  r += (d.*pm)(2) + (pd->*pf)(3);
  r += (n == nullptr) + (pc != pa) + (pm == pf) + (pf != nullptr);
  if (pb) r += 1;
  if (!pf) r += 2;
  return r;
}

int consts() {
  int D::*p = &B::b;
  int (D::*f)(int) const = &B::add;
  D d{};
  return d.*p + (d.*f)(1);
}
