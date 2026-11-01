// [stmt.ambig]: a statement that could be an expression-statement with a
// function-style cast or a declaration is a declaration when it can be
// parsed as one (clang's isCXXDeclarationStatement / TryParseDeclarator).
//@ expect: ast
//@ diag: 23:16 warning "parentheses were disambiguated as a function declaration"
//@ diag: 23:17 note "add a pair of parentheses to declare a variable"
struct T {
  T(); T(int); T(int, int);
  T *operator->();
  void f();
  int m;
  T operator++(int);
  T operator<<(int);
  explicit operator bool() const;
};
int a, c;
// Declarations: the declarator `(e)` is followed by something only a
// declarator can be followed by (`[`, `=`, `,`, `;`, or `(` inside `(*p)`).
void s4() { T(e)[3]; }
void s5() { T(e) = 1; }
void s6() { T(e), b; }
void s7() { T(e), y = 2; }
void s1() { T x(T(a)), y; }
void s3() { T(*p)(int); }
void s12() { T(f) = {1}; }
// Expressions: `->`, `.`, `++`, a second argument, or a binary operator
// after `)` cannot follow a declarator.
void s2() { T(a)->f(); }
void s8() { T(a)->m = 7; }
void s9() { T(a)++; }
void s10() { T(a, 5) << c; }
// The same in the for-init-statement and in a condition.
void s14() { for (T(e); ;) break; }
void s15() { if (T(e) = 1) {} }
void s16() { if (T(a).m) {} }
