// [class.mem.general]/7, [class.pre]/2, [class.default.ctor]/1,
// [expr.prim.req.general], [dcl.spec.auto.general], [module.interface],
// spec/modules.md §11.11 "Forms carried by WP-1360": an imported
// module's noexcept operand naming a later member (with a specialization
// the module instantiates), a class named in its own base clause, an
// implicit constructor named in a later type, a checked concept and a
// function template declared with a deduced return type before its
// definition; under `--bmi-image` they come from the declaration image
// and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++26
//@ module: imgout=Inputs/img-outside-refs.cppm
import imgout;

int main() {
  static_assert(noexcept(ou::counted<int>{}.get()) && noexcept(ou::counted<long>{}.get()));
  static_assert(ou::derived{}.twice() == 42 && ou::other{}.twice() == 2);
  static_assert(ou::fn::go(1, 2) && !noexcept(ou::fn::go(1, 2)));
  static_assert(ou::addable<long> && ou::via(2) == 3);
  return ou::made() == 2 ? 0 : 1;
}
