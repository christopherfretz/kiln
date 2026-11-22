// The written-reference table (spec/ast.md §18), role `member`: `a.m`,
// `p->m()`, qualified members (a member after `.template` has role
// `template`, §18.4), operator-function-ids
// after `.`, explicit destructor calls, and a member found through a
// using-declaration (`found` is the using-shadow).
//@ written-refs: member-names.refs
struct Base {
  int b;
  int get() const { return b; }
};
struct Num : Base {
  using Base::get;
  int v;
  Num operator+(Num o) const { return Num{{0}, v + o.v}; }
  template <class T> T as() const { return T(v); }
  ~Num() {}
};

int members(Num n, Num* p) {
  int s = n.v + p->v + n.Base::b + p->get();
  Num m = n.operator+(*p);
  s += m.template as<int>() + p->as<long>();
  p->~Num();
  return s + m.get();
}
