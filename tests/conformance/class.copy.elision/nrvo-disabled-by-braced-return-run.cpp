// [class.copy.elision]/1.1: a `return` of a braced-init-list is not a
// copy-elision candidate, so the named local returned on another path is
// not given the return slot. kiln elided `result` anyway, so the braced
// return constructed over the live local and its destructor then ran on
// the returned object (a double free; duckdb `Catalog::TryLookupEntry`).
//@ expect: run
//@ strict-diags
//@ stdout: live 0|r 7 9 2 3
#include <cstdio>

static int live = 0;

struct S {
  int *p;
  S(int v) : p(new int(v)) { ++live; }
  S(const S &o) : p(new int(*o.p)) { ++live; }
  S(S &&o) noexcept : p(o.p) { o.p = nullptr; ++live; }
  ~S() { delete p; --live; }
};

struct L {
  int *a;
  S s;
};

__attribute__((noinline)) L before(bool found) {
  L result{nullptr, S(7)};
  if (found)
    return result;
  return {nullptr, S(9)};
}

__attribute__((noinline)) L after(bool found) {
  if (!found)
    return {nullptr, S(2)};
  L result{nullptr, S(3)};
  return result;
}

int main() {
  int a, b, c, d;
  {
    L w = before(true);
    L x = before(false);
    L y = after(false);
    L z = after(true);
    a = *w.s.p, b = *x.s.p, c = *y.s.p, d = *z.s.p;
  }
  std::printf("live %d|r %d %d %d %d\n", live, a, b, c, d);
}
