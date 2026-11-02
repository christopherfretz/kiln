// [class.friend]/11, [class.local]: a friend of a class nested in a local
// class may befriend a member function of the enclosing local class (also
// in a function template), and a local class may befriend a block-scope
// operator declaration; the befriended functions get access.
//@ expect: run
int g() {
  struct L {
    int m() { N n; return n.secret; }
    class N { int secret = 40; friend int m(); };
  };
  return L().m();
}
template <class T> T tt() {
  struct L {
    T m() { N n; return n.secret; }
    class N { T secret = 2; friend T m(); };
  };
  return L().m();
}
struct B { int v; };
int op() {
  bool operator==(B, B);
  class P { int x = 2; friend bool operator==(B, B); public: static int get(P p) { return p.x; } };
  return P::get(P());
}
bool operator==(B a, B b) { return a.v == b.v; }
int main() { return g() + tt<int>() + op() - (B{1} == B{1}) == 43 ? 0 : 1; }
