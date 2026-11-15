// C23 6.7.11p13: a structure is initialized from an expression of a
// compatible type by the simple-assignment constraints; the lvalue
// conversion drops the operand's qualifiers, so a `volatile` or `const`
// structure initializes a plain one (a volatile copy), in a declaration and
// in a braced list (it was rejected as a C++ copy
// construction, "no matching constructor").
//@ expect: ir
//@ args: -std=gnu17
struct S { int a; char b; double d; };
union U { int i; float f; };
void use(struct S *);
void useu(union U *);
void v1(volatile struct S *p) { struct S t = *p; use(&t); }
void v2(volatile struct S *p) { struct S t[2] = {*p, *p}; use(&t[1]); }
void v3(const volatile struct S *p) { struct S t = {0}; t = *p; use(&t); }
void v4(volatile union U *p) { union U u = *p; useu(&u); }
void v5(volatile struct S *p, struct S s) { *p = s; use((struct S *)p); }
struct W { struct S s; int k; };
void v6(volatile struct S *p) { struct W w = {*p, 1}; use(&w.s); }
