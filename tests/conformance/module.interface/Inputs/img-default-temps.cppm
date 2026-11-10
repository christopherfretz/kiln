// Interface unit of module `imgdt` for image-default-temporaries-run.cpp:
// inline and constexpr bodies whose for-range-initializer uses
// a default argument with a temporary. P2718R0 extends that temporary to
// the loop, through a copy of the default argument rebuilt for the use
// (spec/sema.md §7.6a); the importer must keep the extension whether it
// replays this unit or builds the bodies from the declaration image. The
// events go to `trace` (a destructor as minus its value, a loop iteration
// as its value, 0 between cases): a linkage specification such as
// `extern "C" printf` would keep the unit out of the image, and so would a
// range-based `for` in a template (the template case is in
// img-default-temps-tpl.cppm, which replays).
export module imgdt;
namespace dt {
export inline int trace[32]{};
export inline int ntrace = 0;
export inline void put(int v) {
  if (ntrace < 32)
    trace[ntrace++] = v;
}
export struct D {
  int v;
  ~D() { put(-v); }
};
export struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
export inline A mk(const D& = D{7}) { return {}; }
export inline int dv(const D& d = D{8}) { return d.v; }
export inline void loop() {
  for (int x : mk())
    put(x);
  put(0);
  int k = dv();
  put(k);
  put(0);
}
export inline A mku(int, const D& = D{9}) { return {}; }
// A range over a default argument's own temporary: reading it in the
// loop is a constant expression only if the temporary is extended.
export struct R {
  const int* p;
  constexpr const int* begin() const { return p; }
  constexpr const int* end() const { return p + 1; }
};
export constexpr R mkr(const int& x = int{5}) { return {&x}; }
export constexpr int sum() {
  int s = 0;
  for (int x : mkr())
    s += x;
  return s;
}
}
