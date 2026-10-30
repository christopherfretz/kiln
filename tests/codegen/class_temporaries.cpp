// Class temporaries: full-expression destruction, lifetime extension,
// NRVO with a non-trivial destructor, construction into a by-value
// argument, ignored results.
void note(const char*, int) noexcept;

struct T {
  int v;
  T(int x) noexcept : v(x) {}
  T(const T& o) noexcept : v(o.v + 100) {}
  ~T() noexcept { note("~T", v); }
  int get() const noexcept { return v; }
};

T make(int x) noexcept {
  T t(x);
  if (x > 3)
    return t;
  t.v = -x;
  return t;
}

int take(T t) noexcept { return t.v; }
int peek(const T& t) noexcept { return t.v; }

int temps(int k) noexcept {
  int a = T(k).get();
  const T& r = T(k + 1);
  int b = peek(T(k + 2)) + take(T(k + 3));
  T m = make(k);
  make(k + 4);
  T(9);
  return a + r.v + b + m.v;
}
