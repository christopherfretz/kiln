// [namespace.udecl]/14, [over.match.funcs.general]/4: a member operator
// function or conversion function declared after a using-declarator with
// the same parameter-type-list (target type) hides the one it introduced.
// Without the hiding both candidates take the derived class as their
// implicit object parameter and the call would be ambiguous.
//@ expect: run
//@ strict-diags
struct B {
  int operator[](int) const { return 1; }
  int operator()() const { return 1; }
  operator int() const { return 1; }
  operator long() const { return 10; }
};
struct D : B {
  using B::operator[];
  using B::operator();
  using B::operator int;
  using B::operator long;
  int operator[](int) const { return 2; }
  int operator()() const { return 2; }
  operator int() const { return 2; }
};
int main() {
  D d;
  if (d[0] != 2) return 1;
  if (d() != 2) return 2;
  if (d.operator int() != 2) return 3;
  if (d.operator long() != 10) return 4;
  int i = d;
  if (i != 2) return 5;
  return 0;
}
