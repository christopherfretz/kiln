// [expr.sizeof]/1, [expr.alignof]/1: an incomplete (or void, function or
// bit-field) operand is diagnosed, as clang `CheckUnaryExprOrTypeTraitOperand`,
// at the keyword for a type operand and at the operand's expression location
// (its begin, or a member call's member name) for an expression operand.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:9 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 36:9 error "invalid application of 'alignof' to an incomplete type 'void'"
//@ diag: 37:9 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 38:9 error "invalid application of 'alignof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 39:9 error "invalid application of 'sizeof' to a function type"
//@ diag: 40:9 error "invalid application of 'sizeof' to an incomplete type 'char[]'"
//@ diag: 43:15 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 44:16 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 45:16 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 46:18 error "invalid application of '__alignof' to an incomplete type 'void'"
//@ diag: 47:15 error "invalid application of 'sizeof' to bit-field"
//@ diag: 48:16 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 49:18 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 50:15 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 30:8 note "forward declaration of 'I'"
//@ diag: 53:27 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 54:10 note "in instantiation of variable template specialization 'z<void>' requested here"
void f();
struct I;
I &gi();
struct S { int bf : 3; I *p; I &r(); I &operator[](int); } s;

// Type operands: at the keyword.
int a = sizeof(void);
int b = alignof(void);
int c = sizeof(I);
int d = alignof(I);
int e = sizeof (int());
int g = sizeof(char[]);

// Expression operands: at the operand's expression location.
int h = sizeof(f());
int i = sizeof f();
int j = sizeof gi();
int k = __alignof(f());
int l = sizeof(s.bf);
int m = sizeof *s.p;
int n = sizeof s.r();
int o = sizeof(s[1]);

// Instantiated: the pattern's locations.
template<class T> int z = sizeof(T);
int zz = z<void>;
