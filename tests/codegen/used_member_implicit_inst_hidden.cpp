// [temp.inst]/11, GNU `used` under `-fvisibility=hidden`: the `used`
// member functions an implicit instantiation emits are `hidden`, those of
// a class in an unnamed namespace `internal` (see
// used_member_implicit_inst.cpp).
//@ expect: ir
//@ args: -fvisibility=hidden
template <class T> struct S {
  __attribute__((used)) int op() const { return 2; }
  __attribute__((used)) static int sm() { return 1; }
};
S<int> s;
namespace {
template <class T> struct A { __attribute__((used)) int af() { return 5; } };
A<int> a;
}
