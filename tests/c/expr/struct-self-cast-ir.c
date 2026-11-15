// C17 6.5.4p2: a cast names a scalar type; GCC and clang accept a structure
// or union cast to its own type (clang `CheckCStyleCast`: a no-op cast of the
// operand's value). The lvalue conversion drops the operand's qualifiers, so
// a `volatile` or `const` structure casts to the plain type (it was
// rejected as a C++ copy construction). spec/codegen.md "C specifics".
//@ expect: ir
//@ args: -std=gnu17
struct S { int a; char b; double d; };
union U { int i; float f; };
void use(struct S);
void useu(union U);
struct S plain(struct S s) { return (struct S)s; }
void usep(struct S *);
void vol(volatile struct S *p) { struct S t = (struct S)*p; usep(&t); }
void cst(const struct S *p) { use((struct S)*p); }
void un(union U u) { useu((union U)u); }
struct S nested(struct S s) { return (struct S)(struct S)s; }
