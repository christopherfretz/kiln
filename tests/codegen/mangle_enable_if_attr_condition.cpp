// clang `enable_if` attribute (spec/abi.md "enable_if conditions"): each
// condition is part of the function's mangled name (`Ua9enable_ifI
// <template-arg>* E`), so overloads that differ only in their condition
// are distinct symbols. A floating literal mangles as `L<type><bits>E`, a
// temporary (`S{x}.v`) through its materialization, and a braced
// temporary as `tl`. WP-593 (they collided as `XLi0EE` before WP-933);
// compared with clang 19 and 22 (ir_vs_clang).
struct S {
  int v;
  constexpr int get() const { return v; }
};
int f1(double d) __attribute__((enable_if(d > 1.5, ""))) { return 1; }
int f1(double d) __attribute__((enable_if(d < 1.5f, ""))) { return 2; }
int f2(int x) __attribute__((enable_if(S{x}.v == 3, ""))) { return 1; }
int f2(int x) __attribute__((enable_if(S{x}.get() == 4, ""))) { return 2; }
int f3(int x) __attribute__((enable_if(x == 2.0, ""))) { return 1; }
int f3(int x) __attribute__((enable_if(x == 3.0f, ""))) { return 2; }
int use() { return f1(2.0) + f1(1.0) + f2(3) + f2(4) + f3(2) + f3(3); }
