// [stmt.dcl]/3: a jump may not bypass a declaration with initialization
// (or a trivially default-initialized variable with a non-trivial
// destructor); [except.handle]/2: nor enter a try block or handler;
// [stmt.if]/2: nor enter a constexpr / consteval if. An asm goto
// (callbr) may not leave a destructor's scope either. Leaving scopes with
// goto is fine (ok). Diagnostics as clang's JumpScopeChecker, innermost
// protected scope first, later jumps first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:10 error "cannot jump from this goto statement to its label"
//@ diag: 48:5 note "jump bypasses variable with a non-trivial destructor"
//@ diag: 53:3 error "cannot jump from this goto statement to its label"
//@ diag: 55:5 note "jump bypasses variable initialization"
//@ diag: 54:7 note "jump bypasses variable initialization"
//@ diag: 68:3 error "cannot jump from this goto statement to its label"
//@ diag: 64:5 note "jump bypasses initialization of catch block"
//@ diag: 60:3 error "cannot jump from this goto statement to its label"
//@ diag: 61:3 note "jump bypasses initialization of try block"
//@ diag: 76:3 error "cannot jump from switch statement to this case label"
//@ diag: 73:9 note "jump bypasses variable initialization"
//@ diag: 74:3 error "cannot jump from switch statement to this case label"
//@ diag: 73:9 note "jump bypasses variable initialization"
//@ diag: 83:5 error "cannot jump from this asm goto statement to one of its possible targets"
//@ diag: 86:1 note "possible target of asm goto statement"
//@ diag: 82:7 note "jump exits scope of variable with non-trivial destructor"
//@ diag: 90:3 error "cannot jump from this asm goto statement to one of its possible targets"
//@ diag: 93:3 note "possible target of asm goto statement"
//@ diag: 92:9 note "jump bypasses variable initialization"
//@ diag: 98:3 error "cannot jump from this goto statement to its label"
//@ diag: 99:3 note "jump enters controlled statement of constexpr if"
//@ diag: 105:3 error "cannot jump from this goto statement to its label"
//@ diag: 106:9 note "jump enters a statement expression"
//@ diag: 129:3 error "cannot jump from this goto statement to its label"
//@ diag: 130:11 note "jump bypasses variable initialization"
//@ diag: 145:3 error "cannot jump from this goto statement to its label"
//@ diag: 150:7 note "jump bypasses initialization of try block"
//@ diag: 149:11 note "jump bypasses variable initialization"
//@ diag: 147:7 note "jump bypasses variable with a non-trivial destructor"
//@ diag: 158:3 error "cannot jump from this goto statement to its label"
//@ diag: 159:3 note "jump enters controlled statement of consteval if"
struct D { ~D(); };
struct C { C(); };
struct P { int a; };
struct N { N() = default; int a; private: int b; };
void g();
void f1(bool b) {
  if (b) goto l1;
  D d;
l1:
  g();
}
void f2(bool b) {
  goto l2;
  int x = 1;
  C c;
l2:
  g();
}
void f3() {
  goto l3;
  try {
  l3:
    g();
  } catch (...) {
  l4:
    g();
  }
  goto l4;
}
void f4(int n) {
  switch (n) {
  case 0:
    int x = 3;
  case 1:
    g();
  default:
    break;
  }
}
int f5(int x) {
  {
    D d;
    asm goto("" : : "r"(x) : : out);
  }
  return 0;
out:
  return 1;
}
int f6(int x) {
  asm goto("" : : "r"(x) : : in);
  {
    int y = 2;
  in:
    return y;
  }
}
void f7() {
  goto l7;
  if constexpr (true) {
  l7:
    g();
  }
}
void f8() {
  goto l8;
  (void)({ l8: 1; });
}
void ok(bool b) {
  if (b) goto a;
  int x;
  static int s = 1;
  P p;
  N n;
  extern int e;
a:
  try { goto out; } catch (...) { goto out; }
  {
    D d;
    goto out;
  }
out:
  {
  back:
    D d2;
    if (b) goto back;
  }
}
void f9(int x) {
  goto l9;
  if (int y = x) {
  l9:
    g();
  }
}
int f10(int x) {
  const D &r = D();
  asm goto("" : : "r"(x) : : out);
  return 0;
  {
  out:
    return 1;
  }
}
void f11(int x) {
  goto l11;
  {
    D a;
    {
      int b = 1;
      try {
      l11:;
      } catch (...) {}
    }
  }
}
consteval int ce() { return 1; }
void f12() {
  goto l12;
  if consteval {
  l12:
    g();
  }
}
