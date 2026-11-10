// The primary interface of imgp (spec/modules.md §11.11 stage S6): it
// re-exports :a and imports :b, so its image follows a replayed
// preamble of three directives.
export module imgp;
export import :a;
import :b;
export namespace top {
constexpr int answer() { return 42; }
struct Wrap {
  int w;
};
constexpr int unwrap(Wrap x) { return x.w; }
int total(); // defined in img-part-impl.cpp
} // namespace top
