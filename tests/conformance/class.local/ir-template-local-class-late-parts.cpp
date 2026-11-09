// [class.mem.general]/7, [class.local], [temp.inst]/3: the default member
// initializers of a member class of a local class in a function template
// name members declared later in the outermost local class, and a member's
// noexcept-specifier names the function's local enumerator. The
// constructors store the substituted values and the call is a plain
// `call` of a non-throwing function, as clang emits (WP-905).
//@ expect: ir
//@ min-oracle: 22
//@ args: -ftrivial-auto-var-init=uninitialized
int g();
template <class T> int f() {
  enum : bool { FNE = true };
  struct O {
    struct I {
      int a = LATE;
      int b = O::sf();
    };
    enum { LATE = 3 };
    static int sf() noexcept(FNE) { return 5; }
    int h() noexcept(FNE) { return g(); }
  };
  O o;
  typename O::I i;
  return i.a + i.b + o.h();
}
int use() { return f<int>(); }
