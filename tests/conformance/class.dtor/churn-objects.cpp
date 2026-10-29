// [class.ctor], [class.dtor], [expr.new], [expr.delete], [expr.const]/5.20
// + kiln Oven budgets (spec/oven.md "Class benchmarks"): 2 rounds of
// bench/oven/class-churn.cpp: 10000 heap objects with user-provided
// constructors and destructors owning heap buffers, made through by-value
// copies of locals, plus a default-constructed heap array of 10000;
// destroyed in reverse order. The gates are set from the live-object count.
//@ args: -fconstexpr-steps=100000000
//@ oven: 57 steps<=760000 calls<=200002 allocs<=80004 peak_bytes<=655360 peak_arena<=720896
struct Counter {
  int live = 0;
  int made = 0;
};

struct Obj {
  Counter *c;
  int *buf;
  int key;
  constexpr Obj() : c(nullptr), buf(nullptr), key(0) {}
  constexpr Obj(Counter *c, int key) : c(c), buf(new int[2]{key, key + 1}), key(key) {
    ++c->live;
    ++c->made;
  }
  constexpr Obj(const Obj &o) : c(o.c), buf(new int[2]{o.buf[0], o.buf[1]}), key(o.key) {
    ++c->live;
    ++c->made;
  }
  constexpr ~Obj() {
    if (c) {
      --c->live;
      delete[] buf;
    }
  }
  constexpr int sum() const { return buf[0] + buf[1]; }
};

constexpr int touch(Obj o) { return o.sum(); }

constexpr long long churn(int rounds, int n) {
  Counter c;
  long long total = 0;
  for (int r = 0; r < rounds; ++r) {
    Obj **a = new Obj *[n];
    for (int i = 0; i < n; ++i) {
      Obj t(&c, i + r);
      total += touch(t);
      a[i] = new Obj(t);
    }
    Obj *d = new Obj[n];
    for (int i = 0; i < n; ++i) total += a[i]->key + (d[i].c == nullptr);
    delete[] d;
    for (int i = n; i-- > 0;) delete a[i];
    delete[] a;
  }
  return c.live == 0 ? total + c.made : -1;
}

static_assert(churn(2, 10000) == 300100000);
