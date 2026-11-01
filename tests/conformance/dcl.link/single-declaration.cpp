// [dcl.link]/8: a declaration directly contained in a linkage-specification
// (no braces) is treated as if it had `extern`, so `extern "C" int x;`
// declares x without defining it, and a later definition redeclares it.
// Inside braces, `int w;` is a definition.
//@ expect: run
//@ strict-diags
//@ exit: 15
extern "C" int x;
extern "C" int x = 1;
extern "C" { extern int y; }
extern "C" int y = 2;
extern "C" int z;
int z = 3;
extern "C" { int w = 4; }
extern "C" int w;
extern "C" const int k = 5;
int main() { return x + y + z + w + k + 0; }
