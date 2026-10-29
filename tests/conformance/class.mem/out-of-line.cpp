// [class.mfct], [class.mfct.non.static]/2, [class.static]: member functions
// defined outside their class see the members through an implicit this->;
// static members are named through the class or an object.
//@ expect: compile-ok
//@ strict-diags
struct S {
  int x;
  static int count;
  enum E { e1, e2 };
  struct In { int y; };
  int get();
  int twice();
  static int bump();
  int add(int k);
  int make();
};
int S::count = 5;
int S::get() { return x + count + e2; }
int S::twice() { return get() * 2 + S::get(); }
int S::bump() { return ++count + e1; }
int S::add(int k) {
  x += k;
  bump();
  return twice();
}
int S::make() {
  In in;
  in.y = x;
  return in.y;
}
int use(S s, S *p) {
  int a = s.get() + p->twice() + s.add(3) + S::bump() + s.bump() + p->bump();
  return a + s.count + p->count + s.e1 + p->e2 + s.make();
}
