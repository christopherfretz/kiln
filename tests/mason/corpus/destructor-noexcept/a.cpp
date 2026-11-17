void g();
struct D {
  int v = 1;
  ~D() noexcept { g(); }
};
inline int f() {
  D d;
  return d.v;
}
int use_a() { return f(); }
