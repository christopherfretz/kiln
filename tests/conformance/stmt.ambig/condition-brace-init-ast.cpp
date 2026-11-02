// [stmt.pre]/[stmt.ambig]: a condition `T(n){1}` is a declaration with a
// braced initializer (clang's isCXXConditionDeclarationOrInitStatement
// accepts `{` after the declarator), and a `;` after a braced initializer
// at the top level of the parentheses ends an init-statement.
//@ expect: ast
//@ requires: clang>=22
struct T { T(); T(int); explicit operator bool() const; };
void y1() { if (T(n){1}) {} }
void y2() { while (T(n){1}) {} }
void y4() { if (T n{1}) {} }
void y5() { if (int(n){1}; n) {} }
void y6() { switch (int(n){1}) {} }
void y7() { for (;T(n){1};) {} }
void y8() { if (int n{1}) {} }
void y9() {
  if (int n{1}; n) {}
  if (auto g = []{ return 1; }; g()) {}
  switch (int m{2}; m) {}
  if (struct Q { int v; } q{3}; q.v) {}
}
