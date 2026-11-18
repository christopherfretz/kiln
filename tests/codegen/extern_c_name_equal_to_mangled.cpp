// [dcl.link], Itanium C++ ABI 5.1 (WP-1663): a C-linkage name equal to
// the mangled name of a C++ entity of the unit is one symbol, so one
// global: a function declaration, a variable declaration, a function
// definition and a constructor variant under such a name all denote it
// (clang `GetOrCreateLLVMFunction` / `GetOrCreateLLVMGlobal` return the
// entry; a definition replaces a declaration of another type).
// Found running GCC's C++ testsuite.

// A C++ function declared, a C function of its name declared: one
// declaration, each call with its own type.
void f1(int);
extern "C" void _Z2f1i(long);
void use1() { f1(1); _Z2f1i(2); }

// A C variable declared under a function's mangled name: the variable
// is the function (a store to its address).
void f2(int);
extern "C" int _Z2f2i;
void use2() { f2(1); _Z2f2i = 2; }

// The variable first, then the function defined: the definition takes
// the symbol and the variable's uses.
extern "C" int _Z2f3i;
int use3() { return _Z2f3i; }
void f3(int) {}

// The C function defined first, the C++ one only declared and called.
extern "C" void _Z2f4i(int) {}
void f4(int);
void use4() { f4(1); }

// A C variable defined, the C++ function declared and called.
extern "C" int _Z2f5i = 3;
void f5(int);
void use5() { f5(1); }

// A declared constructor whose complete-object variant a C function
// defines: the call is to that function.
struct G { G(int *); };
extern "C" void _ZN1GC1EPi(...) {}
void use6(int *p) { G g(p); }

// A C function declares the complete-object variant of a constructor
// defined later: the alias to the base-object variant replaces it.
struct H { H(int *); };
extern "C" void _ZN1HC1EPi(int *);
void use7(int *p) { _ZN1HC1EPi(p); }
H::H(int *) {}

// A C function defines the complete-object variant of a constructor
// defined later: only the base-object variant is emitted (clang
// `emitConstructorDestructorAlias` leaves a defined symbol alone).
struct K { K(int *); };
extern "C" void _ZN1KC1EPi(int *) {}
K::K(int *) {}

// A destructor's complete-object variant defined by a C function.
struct L { ~L(); };
extern "C" void _ZN1LD1Ev(L *) {}
void use9() { L l; }
