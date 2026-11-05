// The strong definitions that override the weak ones of
// ../weak-structor-run.cpp.
#include "weak-structor.h"
A::A(int v) : x(v) { printf("strong A\n"); }
A::~A() { printf("strong ~A\n"); }
void V::key() {}
V::~V() { printf("strong ~V\n"); }
VB::VB() { printf("strong VB\n"); }
// The complete-object constructor of TO<int> only (`TO<int>::TO(int)`,
// C1): it overrides the weak alias the other unit defines, while that
// unit's base-object variant (C2) stays the weak one.
void to_c1(void *, int) __asm__("_ZN2TOIiEC1Ei");
void to_c1(void *, int) { printf("strong TO C1\n"); }
