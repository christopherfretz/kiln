//@ expect: ast
int i = 1;
const int ci = 2;
int *pi = &i;
auto *p1 = &i;
const auto *p2 = &i;
auto *p3 = &ci;
auto **pp = &pi;
auto &r1 = i;
auto &r2 = ci;
const auto &r3 = i;
auto &&f1 = i;
auto &&f2 = ci;
auto *const cp = &i;
auto *&rp = pi;
int g();
auto *fp = &g;
auto &fr = g;
auto *ret_ptr() { return &i; }
auto &ret_ref() { return i; }
const auto &ret_cref() { return ci; }
auto &&ret_fwd() { return i; }
void use() {
  auto *lp = &i;
  auto &lr = *lp;
  (void)lr;
}
