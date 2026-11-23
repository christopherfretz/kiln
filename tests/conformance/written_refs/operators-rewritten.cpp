// Rewritten and reversed comparisons ([over.match.oper]/3.4, /8, /9;
// spec/ast.md §18.13.3): the entry at the written token names the
// selected `operator==` or `operator<=>`, flagged `rewritten`, and
// `reversed` when the synthesized candidate had its operands swapped.
// When the comparison of the `<=>` result with zero is itself an
// overloaded operator, a second `rewritten` entry at the same token names
// it, after the first (application order); a built-in comparison with
// zero (a standard comparison category) has none. `!=` negates with the
// built-in `!`.
//@ written-refs: operators-rewritten.refs
struct Ord {
  int operator<=>(const Ord &) const;
  bool operator==(const Ord &) const;
};

struct R {};
struct Q {
  int v;
};
R operator<=>(const Q &, int);
bool operator<(R, int);
bool operator<(int, R);
bool operator==(const Q &, int);

void use(Ord a, Ord b, Q q) {
  a < b;
  a != b;
  a == b;
  a <=> b;
  q < 1;
  1 < q;
  q != 1;
  1 == q;
  q <=> 1;
}
