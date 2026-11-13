// Debugger (WP-1372, reported by hod): dynamic classes in a namespace whose
// vtables this unit emits (no key function) are complete types, so
// `print *this` inside an override shows the base and the members instead
// of `<incomplete type>`; a class with a virtual base, too.
//@ expect: debugger
//@ gdb: break shapes::Box::area
//@ gdb: break shapes::Label::length
//@ gdb: run
//@ gdb: print *this
//@ gdb: print this->w + this->h
//@ gdb: ptype shapes::Box
//@ gdb: continue
//@ gdb: print *this
//@ gdb: print this->t + this->n
//@ gdb-check: Breakpoint 1, shapes::Box::area (this=0x
//@ gdb-check: kind = shapes::Kind::Rect
//@ gdb-check: $2 = 7
//@ gdb-check: type = struct shapes::Box : public shapes::Shape {
//@ gdb-check: int w;
//@ gdb-check: Breakpoint 2, shapes::Label::length (this=0x
//@ gdb-check: t = 9
//@ gdb-check: $4 = 14
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --name shapes::Box::area
//@ lldb: run
//@ lldb: frame variable *this
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: (shapes::Box) *this = {
//@ lldb-check: w = 3
namespace shapes {
enum class Kind { Point, Rect };

struct Shape {
  explicit Shape(Kind k) : kind(k) {}
  virtual ~Shape() = default;
  virtual int area() const { return 0; }
  Kind kind;
};

struct Box : Shape {
  Box(int w, int h) : Shape(Kind::Rect), w(w), h(h) {}
  int area() const override {
    int a = w * h;
    return a;
  }
  int w, h;
};

struct Tagged {
  virtual int tag() const { return t; }
  int t = 9;
};

struct Label : virtual Tagged {
  virtual int length() const {
    int len = n;
    return len;
  }
  int n = 5;
};
} // namespace shapes

int measure(const shapes::Shape &s) { return s.area(); }

int main() {
  shapes::Box box(3, 4);
  shapes::Label label;
  return measure(box) + label.length() + label.tag() == 26 ? 0 : 1;
}
