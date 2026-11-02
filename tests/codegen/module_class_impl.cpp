// [class.mfct]/1: in an implementation unit, a class attached to the
// module has non-inline in-class definitions, emitted strongly even when
// unused (Y::f), and a vtable emitted (strongly) only where used; an
// imported interface class's in-class definitions and vtable are only
// declared (its interface unit defines them; spec/modules.md §7).
//@ module: mci=Inputs/mod_class_i.cppm
module mci;
struct X {
  virtual int f() { return 3; }
};
struct Y {
  virtual int f() { return 4; }
};
int z() {
  X x;
  E e;
  return x.f() + e.g() + e.f();
}
