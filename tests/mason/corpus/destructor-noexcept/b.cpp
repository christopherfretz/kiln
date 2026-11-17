void g();
struct D {
  int v = 1;
  ~D() { g(); }
};
inline int f() {
  D d;
  return d.v;
}
int use_b() { return f(); }
