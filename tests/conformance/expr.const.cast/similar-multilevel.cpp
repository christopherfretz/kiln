// [expr.const.cast]/3, [conv.qual]/2: const_cast converts between similar
// pointer types whose levels below the first may be pointers to members
// of the same class or arrays of the same bound; differing innermost
// types, or an array of unknown bound against one of known bound, are
// not allowed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:23 error "const_cast from 'long **' to 'int **' is not allowed"
//@ diag: 17:26 error "const_cast from 'int *(*)[2]' to 'const int *(*)[]' is not allowed"
struct A { int m; };
int A::**mpp; const int A::**c1 = const_cast<const int A::**>(mpp);
int (*pa)[3]; const int (*c2)[3] = const_cast<const int (*)[3]>(pa);
int **pp; const int *const *c3 = const_cast<const int *const *>(pp);
const int *const *cpp; int **c3b = const_cast<int **>(cpp);
long **lpp; auto c4 = const_cast<int **>(lpp);
int *(*pu)[]; auto c5 = const_cast<const int *(*)[]>(pu);
int *(*pk)[2]; auto c6 = const_cast<const int *(*)[]>(pk);
