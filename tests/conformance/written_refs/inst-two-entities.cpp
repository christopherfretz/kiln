// Per-specialization entries (spec/ast.md §18.9): one dependent name, two
// specializations, two entities: the pattern's `area` has one entry
// (§18.1) and one `spec=` line per specialization, each naming the
// member of that specialization's argument ([temp.res.general]/1). A
// rename of `Square::area` edits the pattern's `area` because a line
// names `Square::area`.
//@ written-refs: inst-two-entities.refs
struct Square {
  int area() const { return 1; }
};
struct Circle {
  int area() const { return 2; }
};

template <class Shape> int total(const Shape &s) { return s.area(); }

int run() { return total(Square{}) + total(Circle{}); }
