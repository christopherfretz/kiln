// [expr.const]/16, /22: an immediate invocation returning a reference is
// a glvalue constant expression; its result is the entity it designates,
// not that entity's value. So it may designate a variable that is not
// usable in constant expressions (a non-const global), a reference bound
// to it is bound to that object, and at run time nothing calls the
// consteval function: the address is a constant. The same holds for a
// constexpr reference initialized by one, for an xvalue (`T&&`) result,
// for a class-type result copied from, and for an invocation in a
// default argument (WP-640).
//@ expect: run
//@ strict-diags
//@ exit: 63
struct P { int x, y; };
int h = 3;
constexpr int g = 4;
P mp{7, 8};
constexpr P cp{1, 2};

consteval int& fh() { return h; }
consteval const int& fg() { return g; }
consteval int&& fx() { return static_cast<int&&>(h); }
consteval P& fmp() { return mp; }
consteval const P& fcp() { return cp; }
consteval int& fy() { return mp.y; }

int& gh = fh();
constexpr const int& cg = fg();
static_assert(&cg == &g && cg == 4);
static_assert(&fg() == &g && fg() + 1 == 5);
static_assert(&fh() == &h);
static_assert(&fcp().y == &cp.y && fcp().y == 2);
constexpr int& cy = fy();
static_assert(&cy == &mp.y);

int use(const int& r = fg()) { return &r == &g ? r : 99; }

int main() {
  int& lh = fh();
  const int& lg = fg();
  int&& lx = fx();
  P& lp = fmp();
  P copy = fcp();
  lh = 5;
  lp.x = 9;
  int bits = 0;
  bits |= (&gh == &h && h == 5) << 0;
  bits |= (&lg == &g && lg == 4) << 1;
  bits |= (&lx == &h) << 2;
  bits |= (&lp == &mp && mp.x == 9) << 3;
  bits |= (copy.x == 1 && copy.y == 2) << 4;
  bits |= (use() == 4 && fh() == 5) << 5;
  return bits;
}
