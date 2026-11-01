//@ expect: ast
//@ oracle: clang
void f() { void *p = &&l; l: goto *p; }
