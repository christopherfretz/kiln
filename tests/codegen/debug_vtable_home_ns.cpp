// Debug info (WP-1372): vtable homing of dynamic classes in a namespace with
// no key function, whose linkonce_odr vtables this unit emits (the
// constructors are called): Shape, Box (an override of an inline virtual
// function) and Label (a virtual base) are definitions with elements,
// vtableHolder, DW_TAG_inheritance (DIFlagVirtual for Label) and _vptr$
// members, as clang's completeClassData from GenerateClassData.
//@ args: -g
namespace geo {
namespace detail {
enum class Kind { Point, Box };
}
struct Shape {
  explicit Shape(detail::Kind k) : kind(k) {}
  virtual ~Shape() {}
  virtual int area() const { return 0; }
  detail::Kind kind;
  static int made;
};
int Shape::made = 0;
struct Box : Shape {
  Box(int w, int h) : Shape(detail::Kind::Box), w(w), h(h) { made++; }
  int area() const override { return w * h; }
  int w, h;
};
struct Tagged {
  virtual int tag() const { return 7; }
  int t = 0;
};
struct Label : virtual Tagged {
  virtual int length() const { return n; }
  int n = 3;
};
} // namespace geo
int measure(const geo::Shape &s) { return s.area(); }
int main() {
  geo::Box b(3, 4);
  geo::Label l;
  return measure(b) + l.tag() + l.length();
}
