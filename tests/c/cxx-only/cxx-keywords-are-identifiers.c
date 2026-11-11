// C17 6.4.1, 6.4.2: the C++ keywords are identifiers in C.
//@ expect: compile-ok
//@ strict-diags
int class = 1, new = 2, delete = 3, template = 4, this = 5, private = 6;
int namespace(int try) { return try; }
struct S { int public, operator, virtual; };
int main(void) { int catch = class + new; return catch - 3 + namespace(0); }
