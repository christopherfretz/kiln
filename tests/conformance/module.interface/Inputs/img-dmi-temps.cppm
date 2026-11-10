// Interface unit of module `imgdmi` for
// image-default-member-init-temporaries-run.cpp: inline and constexpr
// bodies that use a default member initializer holding a temporary, in a
// for-range-initializer (P2718R0 extends it to the loop) and for a
// reference member of a variable's aggregate (CWG1815 extends it with the
// variable). Both go through a copy of the member's initializer rebuilt
// for the use (spec/sema.md §7.6a), which the declaration image must carry.
// The events go to `trace` (a destructor as minus its value, a loop
// iteration as its value, 0 between cases).
export module imgdmi;
namespace dm {
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
export struct Agg {
  int x = D{4}.v;
};
export struct R {
  const D& r = D{7};
};
export inline A mk(const Agg&) { return {}; }
export inline void loop() {
  for (int x : mk(Agg{}))
    put(x);
  put(0);
  {
    R r{};
    put(r.r.v);
  }
  put(0);
}
export struct T {
  int* log;
  int id;
  constexpr ~T() { *log = *log * 10 + id; }
};
export struct CAgg {
  int* log;
  int x = T{log, 4}.id;
};
export struct Rng {
  int a[2] = {1, 2};
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
export constexpr Rng mkc(const CAgg&) { return {}; }
export constexpr int cloop() {
  int log = 0;
  for (int v : mkc(CAgg{&log}))
    log = log * 10 + v;
  return log;
}
}
