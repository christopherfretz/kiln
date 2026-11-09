// [stmt.expand]/5, [stmt.break]/1, [stmt.cont]/1, [stmt.return],
// [stmt.goto] (GNU statement expressions): a jump in a statement expression
// in an expansion statement's header. The init-statement, an iterating
// range and a destructuring initializer run once, before the first element
// (the statement's preamble), so their break/continue target the enclosing
// loop or switch. An element of an expansion-init-list initializes the
// for-range-declaration inside that element's block ([stmt.expand]/5.1, `{
// decl = E_i; body }`), so a jump there ends the element (continue) or the
// whole expansion statement (break), as clang 23's codegen and GCC 16.1 do.
// An outer expansion statement is the loop of an inner one's header. return
// and goto leave as anywhere else. Destructors run on every path. The
// constexpr functions run here at run time (their constant evaluation is
// header-jump-consteval*.cpp). GCC 16.1 agrees (the oracle: clang 22 has
// no P1306). WP-1081; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: 69 1900 6 0 | 1503 1001 | 2120 1030 3260 | 221 221 | -1 13 33 11133
//@ stdout: x1 x3 ~D0 m ~D10 | x1 x2 x3 ~D0 m y20 y30 ~D30 ~D20 ~D10 e x1 x3 ~D1 m ~D11 | x1 x2 x3 ~D0 m y20 y30 ~D30 ~D20 ~D10 e x1 x2 x3 ~D1 m y21 y31 ~D31 ~D21 ~D11 e | end
extern "C" int printf(const char*, ...);
struct P { int a, b; };

// init-statement: continue/break of the enclosing for.
constexpr int init_jump(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (int i = ({ if (k == 1) continue; 0; }); auto x : {1, 2}) { s += x + i; }
    template for (int i = ({ if (k == 3) break; 0; }); auto x : {10, 20}) { s += x + i; }
  }
  return s;
}
// expansion-init-list element: continue ends that element only.
constexpr int element_continue(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (auto x : {1, ({ if (k == 1) continue; 2; }), 4}) { s += x * 100; }
  }
  return s;
}
// destructuring initializer: continue of the enclosing for.
constexpr int destructuring_continue(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (auto x : ({ if (k == 1) continue; P{1, 2}; })) { s += x; }
  }
  return s;
}
// A switch around: break in the init-statement leaves the switch.
int switch_init(int n) {
  int s = 0;
  switch (n) {
  case 1:
    template for (int i = ({ break; 0; }); auto x : {1, 2}) { s += x + i; }
    s += 1000;
  }
  return s;
}
// expansion-init-list element: break leaves the expansion statement only.
int element_break(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (auto x : {1, ({ if (k == 1) break; 2; }), 4}) { s += x * 100; }
    s += 1;
  }
  return s;
}
// ... even when only a switch encloses it (the switch is what makes the
// break valid).
int element_break_switch(int n) {
  int s = 0;
  switch (n) {
  case 1:
    template for (auto x : {1, ({ if (n == 1) break; 2; }), 4}) { s += x; }
    s += 1000;
  }
  return s;
}
// Nested: the outer expansion statement is the loop of the inner header.
int nested_init(int n) {
  int s = 0;
  template for (auto y : {1, 2, 3}) {
    template for (int i = ({ if (y == n) continue; 0; }); auto x : {10, 20}) { s += x * y + i; }
    s += 1000;
  }
  return s;
}
int nested_destructuring(int n) {
  int s = 0;
  template for (auto y : {1, 2, 3}) {
    template for (auto x : ({ if (y == n) break; P{10, 20}; })) { s += x * y; }
    s += 1000;
  }
  return s;
}
int nested_element(int n) {
  int s = 0;
  template for (auto y : {1, 2, 3}) {
    template for (auto x : {10, ({ if (y == n) break; 20; }), 30}) { s += x * y; }
    s += 1000;
  }
  return s;
}
// A template: the pattern's jumps keep their targets in each instantiation.
template <class T> T in_template(T n) {
  T s = 0;
  for (T k = 0; k < n; ++k) {
    template for (auto x : {T(1), ({ if (k == 1) continue; T(2); }), T(4)}) { s += x; }
    template for (T i = ({ if (k == 2) break; T(0); }); auto x : {T(1)}) { s += 100 + i + x; }
  }
  return s;
}
// return and goto.
int ret_goto(int n) {
  int s = 0;
  template for (int i = ({ if (n == 1) return -1; 0; }); auto x : {1, 2}) { s += x + i; }
  template for (auto x : {1, ({ if (n == 2) return s; 2; })}) { s += x * 10; }
  template for (auto x : ({ if (n == 3) goto out; P{5, 6}; })) { s += x * 100; }
  s += 10000;
out:
  return s;
}

// Destructors: the init-statement's variable lives to the end of the
// expansion statement and is destroyed by a jump out of the range; a
// destructuring initializer's temporary is destroyed after its elements.
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct Q { D a, b; };
void dtors(int n) {
  for (int k = 0; k < 2; ++k) {
    template for (D d{k}; auto x : {1, ({ if (k == n) continue; 2; }), 3}) { printf("x%d ", x); }
    printf("m ");
    template for (D d{k + 10}; auto&& x : ({ if (k == n) break; Q{D{k + 20}, D{k + 30}}; })) {
      printf("y%d ", x.v);
    }
    printf("e ");
  }
  printf("| ");
}

int main() {
  printf("%d %d %d %d | ", init_jump(5), element_continue(3), destructuring_continue(3),
         switch_init(1));
  printf("%d %d | ", element_break(3), element_break_switch(1));
  printf("%d %d %d | ", nested_init(2), nested_destructuring(2), nested_element(2));
  printf("%d %ld | ", in_template(4), in_template(4L));
  printf("%d %d %d %d\n", ret_goto(1), ret_goto(2), ret_goto(3), ret_goto(4));
  dtors(0);
  dtors(1);
  dtors(5);
  printf("end\n");
}
