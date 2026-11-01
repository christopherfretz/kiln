// [expr.cond]/4: operands of different types, one of class type, are
// unified by converting one to match the other: a derived lvalue binds
// to a reference to its base, a class converts to a scalar through a
// conversion function, a scalar converts to a class by a constructor.
//@ expect: run
//@ strict-diags
struct Base { int b = 1; };
struct Der : Base { int d = 2; Der() { b = 5; } };
struct Conv { int v; operator int() const { return v; } };
struct Wrap { int v; Wrap(int x) : v(x * 10) {} };
struct Ref { int& r; operator int&() const { return r; } };
int pick(bool c) {
  Base gb; Der gd;
  Base& r = c ? gb : gd;           // Base lvalue
  r.b += 1;
  int x = c ? Conv{7} : 3;         // int prvalue
  Wrap w = c ? Wrap(1) : 4;        // Wrap prvalue
  int i = 0;
  (c ? i : Ref{i}) = 9;            // int lvalue through operator int&
  return gb.b + gd.b + x + w.v + i;
}
int main() {
  // c=true:  gb.b=2 gd.b=5 x=7 w=10 i=9 -> 33
  // c=false: gb.b=1 gd.b=6 x=3 w=40 i=9 -> 59
  return pick(true) == 33 && pick(false) == 59 ? 0 : 1;
}
