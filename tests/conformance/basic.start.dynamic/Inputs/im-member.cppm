// Module of the inline-member-module-* tests.
export module im;
int calls = 0;
int bump(int v) {
  ++calls;
  return v;
}
export int init_calls() { return calls; }
export struct A {
  static inline int x = bump(7);
};
export int module_x() { return A::x; }
