// [class.default.ctor]/2: the deleted-default-constructor notes name a
// class template specialization with its template arguments (clang
// `getNameForDiagnostic`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:8 error "call to implicitly-deleted default constructor of 'G<int>'"
//@ diag: 15:31 note "default constructor of 'G<int>' is implicitly deleted because base class 'F<int>' has a deleted default constructor"
//@ diag: 14:31 note "'F' has been explicitly marked deleted here"
//@ diag: 18:12 error "call to implicitly-deleted default constructor of 'H<char, 2>'"
//@ diag: 17:40 note "default constructor of 'H<char, 2>' is implicitly deleted because field 'm' has no default constructor"
//@ diag: 21:10 error "call to implicitly-deleted default constructor of 'P<int *>'"
//@ diag: 20:39 note "default constructor of 'P<int *>' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
struct M { M(int); };
template <class T> struct F { F() = delete; };
template <class T> struct G : F<T> {};
G<int> g;
template <class T, int N> struct H { M m; };
H<char, 2> h;
template <class T> struct P;
template <class T> struct P<T *> { T &r; };
P<int *> p;
