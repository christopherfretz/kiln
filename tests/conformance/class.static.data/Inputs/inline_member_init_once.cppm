// The module for ../inline-member-module-init-once-run.cpp.
module;
extern "C" int printf(const char *, ...);
export module inline_member_init_once;
export struct Counted {
  int id;
  Counted(int i) : id(i) { printf("init %d\n", i); }
};
export struct A {
  static inline Counted c{1};
  static constexpr int n = 2;
  static inline int k = printf("init k\n");
};
constexpr int A::n;
export int touch() { return A::c.id + A::n + A::k; }
