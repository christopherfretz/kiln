// Interface unit of module `imgs` for image-gate-scope-run.cpp and
// image-gate-overload-run.cpp (spec/modules.md §11.11 "Part 2a", "ODR
// merging", WP-1162): a global function, a class with members and a
// namespace. Its image gates the global-scope names `twice`, `Box` and
// `geo2` only.
export module imgs;
export int twice(int x) { return 2 * x; }
export struct Box {
  int v;
  constexpr int get() const { return v; }
};
export namespace geo2 {
constexpr int area(int w, int h) { return w * h; }
int count = 3;
} // namespace geo2
