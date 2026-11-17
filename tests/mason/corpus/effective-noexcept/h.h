// a member's noexcept(false) makes the implicit destructor potentially throwing:
// a synthesized member's effective noexcept is not hashed
struct M {
  ~M() SPEC;
};
struct X {
  M m;
  int v = 1;
};
inline int f(int a) {
  X x;
  return x.v + a;
}
