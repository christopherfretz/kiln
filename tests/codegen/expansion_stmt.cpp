// [stmt.expand] (P1306R5): an expansion statement lowers to its
// expansions in order, each a block of its own with no loop back-edge;
// `break` branches past the last expansion and `continue` to the end of
// the current one. Covers the enumerating, iterating (a constexpr array)
// and destructuring (an aggregate) forms.
// (clang 19 and 22 have no expansion statements: compared from clang 23,
// kiln's IR only verifies under an older oracle.)
//@ min-oracle: 23
void g(int);

void enumerating(int n) {
  template for (auto x : {1, 2, 3}) {
    if (x == n) continue;
    if (x > n + 1) break;
    g(x);
  }
}

constexpr int arr[] = {4, 5};
struct A { int a; long b; };
void iterating_and_destructuring(A s) {
  template for (constexpr int x : arr) { g(x); }
  template for (auto x : s) { g(int(x)); }
}

void nested() {
  template for (auto x : {1, 2}) {
    template for (auto y : {10, 20}) { g(x * y); }
  }
}
