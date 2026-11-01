// [[no_unique_address]] members: a potentially-overlapping class member is
// lowered as its base-subobject type (clang CGRecordLowering), also when
// the member is instantiated from a template.
struct Empty {};
struct E2 { };
struct Tail { int a; char b; Tail() : a(1), b(2) {} };
struct A {
  [[no_unique_address]] Empty e;
  int x;
};
struct B {
  [[no_unique_address]] Tail t;
  char c;
};
struct C {
  int x;
  [[no_unique_address]] Empty e;
  [[no_unique_address]] E2 f;
};
template <class T, class U> struct Pair {
  [[no_unique_address]] T t;
  [[no_unique_address]] U u;
};
struct Opt { union { char d; int v; }; bool engaged; };
struct D { [[no_unique_address]] Opt o; char extra; };
int use(A &a, B &b, C &c, D &d) {
  Pair<Empty, int> p{{}, 3};
  Pair<Tail, char> q;
  q.u = 4;
  B bb;
  bb.c = 5;
  return a.x + b.c + b.t.a + c.x + p.u + q.u + q.t.b + d.extra + bb.t.a + sizeof(B) + sizeof(D);
}
