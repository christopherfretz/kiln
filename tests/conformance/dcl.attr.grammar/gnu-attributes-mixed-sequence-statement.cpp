// [dcl.attr.grammar]/1 with GNU attributes: a statement may start with a
// sequence that mixes `__attribute__((...))` and `[[...]]` specifiers in
// any order, including empty ones; followed by `;` it is an
// attribute-declaration (`fallthrough`, `assume`), and it may also
// precede a label, a `case` label's statement, a compound statement, a
// `return` and a declaration (GCC 16.1, clang 22). Found running GCC's C++
// testsuite (WP-1466).
//@ expect: run
//@ std: c++23
//@ requires: clang>=22
//@ strict-diags
int f(int x) {
  switch (x) {
  case 0:
    x++;
    __attribute__((fallthrough)) [[]];
  case 1:
    x++;
    [[]] __attribute__((fallthrough));
  case 2:
    x++;
    __attribute__(()) [[]] __attribute__((fallthrough)) [[]];
  case 3:
    x++;
    __attribute__((fallthrough)); case 4: x++;
    break;
  }
  __attribute__(()) [[]] __attribute__((assume(x > 0))) [[]];
  [[]] __attribute__((assume(x > 0)));
  __attribute__(());
  __attribute__(()) [[]];
  [[]] __attribute__(()) int b = 1;
  __attribute__(()) [[]] int c = 2;
  __attribute__((unused)) lab: x++;
  [[]] __attribute__((unused)) lab2: x++;
  __attribute__(()) [[]] lab3: x++;
  __attribute__(()) [[]] { x++; }
  __attribute__(()) [[]] return x + b + c;
}
int main() { return f(0) == 12 ? 0 : 1; }
