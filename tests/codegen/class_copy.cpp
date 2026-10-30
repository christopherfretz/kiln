// Copies: trivial copies are memcpys, defaulted copy constructors memcpy
// runs of trivially-copyable fields, user-provided copies are calls.
void note(const char*, int) noexcept;

struct Pod { int a; double b; char c; };
struct Sub {
  int s;
  Sub(int x) noexcept : s(x) {}
  Sub(const Sub& o) noexcept : s(o.s) { note("Sub copy", s); }
  Sub& operator=(const Sub& o) noexcept { s = o.s; note("Sub assign", s); return *this; }
};
struct Mixed {
  int a;
  int b;
  Sub sub;
  long c;
  long d;
  Mixed(int x) noexcept : a(x), b(x), sub(x), c(x), d(x) {}
  Mixed(const Mixed&) = default;
  Mixed& operator=(const Mixed&) = default;
};

int copies(Pod p, int k) noexcept {
  Pod q = p;
  Pod r = {q.a, 1.0, 'x'};
  Mixed m(k);
  Mixed n = m;
  return q.a + r.a + n.a + (int)n.d + n.sub.s;
}
