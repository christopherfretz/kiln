//@ expect: ast
struct S {
  int x;
  int get(int k);
};
int S::get(int k) { return x + k; }
int use(S s, S *p) {
  auto pm = &S::x;
  auto pf = &S::get;
  return s.*pm + p->*pm + (s.*pf)(1) + (p->*pf)(2);
}
