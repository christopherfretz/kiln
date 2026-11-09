// Interface unit of module `imga.gate` for image-gate-dotted-run.cpp
// (spec/modules.md §11.11 "Part 2a", "ODR merging"): a dotted module name
// whose components name the nested namespaces it declares.
export module imga.gate;
export namespace imga {
namespace gate {
constexpr int tri(int n) { return n * (n + 1) / 2; }
consteval int cube(int x) { return x * x * x; }
inline int mix(int a, int b = 10) { return a * 100 + b; }
} // namespace gate
int outer(int x) { return gate::tri(x) + 1; }
} // namespace imga
