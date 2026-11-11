// The header of img-attrs.cppm: declarations with attributes.
namespace ia {
[[nodiscard]] inline int must(int x) { return x + 1; }
[[deprecated("use must")]] inline int old(int x) { return x; }
struct [[nodiscard]] Res {
  int v;
};
inline Res make_res() { return {3}; }
[[gnu::abi_tag("v1")]] int tagged(int x);
struct [[gnu::abi_tag("t2")]] Tagged {
  int v;
};
Tagged make_tagged(int x);
__attribute__((visibility("hidden"))) int hidden_fn(int x);
[[gnu::noinline]] int slow(int x);
template <class T> struct B {
  __attribute__((exclude_from_explicit_instantiation)) int ex() { return 1; }
  int in() { return 2; }
};
} // namespace ia
