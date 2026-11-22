// The written-reference table (spec/ast.md §18.3): `enclosing` is the
// innermost declaration containing the name. A default argument's names
// are enclosed by the parameter and a default member initializer's by
// the field, also when they are parsed after the class
// ([class.mem.general]/7), and each has one entry at its written site
// however often it is used. A lambda's body is enclosed by its call
// operator, a local class's member function by that function, and an
// out-of-line definition's qualifier by the declaration it defines.
//@ written-refs: enclosing.refs
namespace defaults {
int base = 7;
int scale(int v, int by = base) { return v * by; }
struct Widget {
  int width = base + extra;
  int height = scale(width);
  int resize(int to = limit()) const;
  static int limit() { return 100; }
  static constexpr int extra = 1;
};
} // namespace defaults

int defaults::Widget::resize(int to) const { return to + width; }

int uses() {
  defaults::Widget w;
  struct Local {
    int get(int k = defaults::base) { return k; }
  } l;
  auto twice = [&w] { return w.height * 2; };
  return defaults::scale(1) + defaults::scale(2) + w.resize() + w.resize() + l.get() + twice();
}
