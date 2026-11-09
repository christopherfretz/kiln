// Interface unit of module `imgm` for image-gate-module-name-run.cpp and
// image-gate-clash-run.cpp (spec/modules.md §11.11 "Part 2a", "ODR
// merging"): the usual layout, a namespace named like its module. The
// importer's `import imgm;` names the module but declares nothing, so
// under `--bmi-image` the importer builds the unit from its image.
export module imgm;
export namespace imgm {
constexpr int fact(int n) {
  int r = 1;
  for (int i = 2; i <= n; ++i)
    r *= i;
  return r;
}
consteval int sq(int x) { return x * x; }
inline int next() {
  static int calls = 0;
  return ++calls;
}
int twice(int x) { return 2 * x; }
constexpr int kSeed = fact(4) + 1;
} // namespace imgm
