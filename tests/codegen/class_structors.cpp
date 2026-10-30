// Constructors, destructors and member functions of non-polymorphic
// classes: C1/C2 aliases for out-of-line structors, RAUW for inline
// ones, base and member destruction, member calls.
void note(const char*, int) noexcept;

struct Counter {
  int n;
  Counter() noexcept;
  Counter(int v) noexcept;
  ~Counter() noexcept;
  void bump() noexcept;
  int get() const noexcept { return n; }
};
Counter::Counter() noexcept : n(0) {}
Counter::Counter(int v) noexcept : n(v) { note("ctor", v); }
Counter::~Counter() noexcept { note("dtor", n); }
void Counter::bump() noexcept { ++n; }

struct Inline {
  int a, b;
  Inline(int x) noexcept : a(x), b(x + 1) {}
  ~Inline() noexcept { note("~Inline", a); }
};

struct Base {
  int x;
  Base(int v) noexcept : x(v) {}
  ~Base() noexcept { note("~Base", x); }
};
struct Derived : Base {
  Counter c;
  int y = 7;
  Derived(int v) noexcept : Base(v), c(v * 2) {}
};

int use(Counter& c) noexcept { c.bump(); return c.get(); }

int run(int k) noexcept {
  Counter a;
  Counter b(k);
  Inline i(k);
  Derived d(k);
  Base* p = &d;
  int r = use(b) + i.b + p->x + d.y;
  a.bump();
  return r + a.get();
}
