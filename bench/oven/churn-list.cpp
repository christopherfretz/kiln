//@ args: -fconstexpr-steps=100000000
// Oven allocation churn (spec/oven.md "Churn benchmarks"): 1000 rounds of:
// build a 100-node singly linked list (16-byte nodes), sum it, destroy it.
// Freed nodes must be reused: the heap peaks at one list, not 1000. Only
// new/delete of scalars and aggregates. The gated copy is
// tests/conformance/expr.new/churn-list.cpp.

struct Node {
  int v;
  Node *next;
};

// Builds an n-node list, sums it, and destroys it.
constexpr long long round_trip(int n, int seed) {
  Node *head = nullptr;
  for (int i = 0; i < n; ++i) head = new Node{seed + i, head};
  long long s = 0;
  for (Node *p = head; p; p = p->next) s += p->v;
  while (head) {
    Node *next = head->next;
    delete head;
    head = next;
  }
  return s;
}

constexpr long long churn(int rounds, int n) {
  long long t = 0;
  for (int r = 0; r < rounds; ++r) t += round_trip(n, r);
  return t;
}

static_assert(churn(1000, 100) == 54900000);
