// [expr.new], [expr.delete], [expr.const]/5.20 + kiln Oven budgets
// (spec/oven.md "Churn benchmarks"): 16 rounds of: build a complete binary
// tree of 4095 nodes (24 bytes each), count it, destroy it in post-order.
// The same program as bench/oven/churn-tree.cpp; the gates are the spec's
// design targets, set from the live-object count.
//@ args: -fconstexpr-steps=100000000
//@ oven: 39 allocs<=65520 peak_bytes<=131072 peak_arena<=196608
struct Tree {
  Tree *left;
  Tree *right;
  int key;
};

constexpr Tree *build(int depth, int key) {
  if (depth == 0) return nullptr;
  return new Tree{build(depth - 1, 2 * key), build(depth - 1, 2 * key + 1), key};
}

constexpr int count(const Tree *t) { return t ? 1 + count(t->left) + count(t->right) : 0; }

// Post-order: children before their parent.
constexpr void destroy(Tree *t) {
  if (!t) return;
  destroy(t->left);
  destroy(t->right);
  delete t;
}

constexpr int churn(int rounds, int depth) {
  int total = 0;
  for (int r = 0; r < rounds; ++r) {
    Tree *t = build(depth, 1);
    total += count(t);
    destroy(t);
  }
  return total;
}

static_assert(churn(16, 12) == 16 * 4095);
