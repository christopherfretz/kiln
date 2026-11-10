// An implementation partition of imgp (spec/modules.md §11.11 stage
// S6): nothing it declares is exported, and only imgp's units see it.
module imgp:b;
namespace pb {
struct Hidden {
  int v;
};
constexpr int triple(int x) { return 3 * x; }
} // namespace pb
