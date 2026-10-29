//@ expect: ast
struct S {
  int x;
  static int count;
  int get(int k);
  static int bump();
};
int S::count = 5;
int S::get(int k) { return x + k + count; }
int S::bump() { return ++count; }
int use(S s, S *p) {
  return s.get(1) + p->get(s.x) + S::bump() + s.bump() + s.count + p->count;
}
