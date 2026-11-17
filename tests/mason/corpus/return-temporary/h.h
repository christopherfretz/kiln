// copy elision is mandatory from C++17
struct T {
  int v = 1;
  T() {}
  T(const T &o) : v(o.v) {}
};
inline T f() { return T(); }
