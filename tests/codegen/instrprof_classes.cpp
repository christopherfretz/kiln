// Source-based coverage counters (WP-941) for implicit and template code:
// constructors and destructors (clang counts the destructor's body under
// the D2 variant only and gives a delegating D1 no record), a lambda's
// call operator, function template specializations (one record per
// specialization, named by its mangled name), virtual functions, and
// `static` (internal) functions, whose `__profn_` name is prefixed with
// the source file name. A consteval function gets no record.
//@ args: -fprofile-instr-generate -Xclang -disable-llvm-passes
struct Base {
  int v;
  Base(int x) : v(x > 0 ? x : 0) {}
  virtual ~Base() {
    if (v)
      v = 0;
  }
  virtual int get() const { return v; }
};

struct Derived : Base {
  Derived() : Base(3) {}
  int get() const override { return v * 2; }
};

template <class T> T twice(T t) { return t + t; }

consteval int folded(int n) { return n < 2 ? 1 : n * folded(n - 1); }

static int local_helper(int n) { return n & 1 ? n : -n; }

int use(int n) {
  Derived d;
  auto f = [n](int k) { return k > n ? k : n; };
  int r = f(d.get()) + twice(n) + int(twice(1.5)) + folded(4);
  return r + local_helper(n);
}
