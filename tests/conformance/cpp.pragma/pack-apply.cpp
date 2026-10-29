// [cpp.pragma]: '#pragma pack(N)' caps the alignment of the members of the
// classes defined while it is in effect (clang's MaxFieldAlignment);
// push/pop with and without labels; the value is taken where the class
// definition begins.
//@ expect: compile-ok
#pragma pack(push, 1)
struct A { char c; int i; };
#pragma pack(push, lbl, 2)
struct B { char c; int i; double d; };
#pragma pack(push, 8)
#pragma pack(pop, lbl)
struct C { char c; int i; };
#pragma pack(pop)
struct D { char c; int i; };
#pragma pack(4)
struct E { char c; double d; };
#pragma pack()
struct F { char c; double d; };
#pragma pack(push, 1)
struct O {
  char c;
#pragma pack(pop)
  struct In { char c; int i; } in;
};
static_assert(sizeof(A) == 5 && alignof(A) == 1);
static_assert(sizeof(B) == 14 && alignof(B) == 2);
static_assert(sizeof(C) == 5);
static_assert(sizeof(D) == 8);
static_assert(sizeof(E) == 12 && alignof(E) == 4);
static_assert(sizeof(F) == 16);
static_assert(sizeof(O::In) == 8 && sizeof(O) == 9);
