// Static members, delegating constructors, reference members, value
// initialization, derived-to-base pointer conversions with a null check,
// and arrays of objects with destructors.
void note(const char*, int) noexcept;

struct S {
  static int count;
  static int next() noexcept { return ++count; }
  int v;
  S() noexcept : S(next()) {}
  S(int x) noexcept : v(x) { note("S", x); }
  ~S() { note("~S", v); }
  int get() const noexcept { return v; }
};
int S::count = 0;

struct Ref {
  int& r;
  const S& s;
  Ref(int& a, const S& b) noexcept : r(a), s(b) {}
  int sum() const noexcept { return r + s.get(); }
};

struct Zero {
  int a;
  long b;
  Zero() = default;
};

struct A { int a; };
struct B { long b; };
struct C : A, B { int c; };

B* to_b(C* c) { return c; }
B& to_b_ref(C& c) { return c; }
const A* to_a(const C* c) { return c; }

int statics(S& s) {
  int a = S::next();
  int b = s.next();
  return a + b + S::count;
}

int arrays() {
  S arr[3];
  S two[2][2];
  return arr[1].get() + two[1][0].get();
}

int refs(int k) {
  S s(k);
  Ref r(k, s);
  return r.sum();
}

long zeros() {
  Zero z{};
  Zero w = Zero();
  C c{};
  return z.b + w.a + c.c;
}
