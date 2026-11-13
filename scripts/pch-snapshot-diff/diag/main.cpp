#include <vector>
int f() {
  int unused = 1;
  return pfx::needs_member(3);
}
pfx::Checked<char> c;
pfx::Widget w = 4;
int g() { return pfx::old_square(1) + undeclared_name; }
