// [dcl.stc]/4, [basic.link]/3, [basic.link]/6: a static variable after a declaration with external linkage (a block-scope extern included), and a non-static one after a static first declaration
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:27 error "static declaration of 'a1' follows non-static declaration"
//@ diag: 24:12 note "previous declaration is here"
//@ diag: 25:20 error "static declaration of 'a2' follows non-static declaration"
//@ diag: 25:5 note "previous definition is here"
//@ diag: 27:20 error "non-static declaration of 'a4' follows static declaration"
//@ diag: 27:12 note "previous definition is here"
//@ diag: 28:24 error "non-static declaration of 'a5' follows static declaration"
//@ diag: 28:12 note "previous definition is here"
//@ diag: 30:12 error "static declaration of 'b1' follows non-static declaration"
//@ diag: 29:30 note "previous declaration is here"
//@ diag: 32:12 error "static declaration of 'b2' follows non-static declaration"
//@ diag: 31:30 note "previous declaration is here"
//@ diag: 35:5 error "non-static declaration of 'b3' follows static declaration"
//@ diag: 33:12 note "previous definition is here"
//@ diag: 37:26 error "static declaration of 'b4' follows non-static declaration"
//@ diag: 36:44 note "previous declaration is here"
//@ diag: 41:35 error "non-static declaration of 'c3' follows static declaration"
//@ diag: 41:27 note "previous declaration is here"
//@ diag: 42:31 error "static declaration of 'c4' follows non-static declaration"
//@ diag: 42:12 note "previous definition is here"
extern int a1; static int a1;
int a2; static int a2;
static int a3; extern int a3;
static int a4; int a4;
static int a5 = 1; int a5 = 2;
inline int g1() { extern int b1; return b1; }
static int b1;
inline int g2() { extern int b2; return b2; }
static int b2 = 3;
static int b3;
inline int g3() { extern int b3; return b3; }
int b3;
namespace n { inline int g4() { extern int b4; return b4; } }
namespace n { static int b4; }
// Internal linkage already: no conflict.
namespace { extern int c1; static int c1; }
extern const int c2; static int f2() { extern const int c2; return c2; }
static int c3; extern int c3; int c3;
inline int c4 = 1; static int c4;
