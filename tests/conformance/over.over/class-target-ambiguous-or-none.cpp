// [over.over]/1, [over.match.copy], [temp.deduct.call]/6, [class.conv.ctor]/2:
// an overload set two constructors both accept is ambiguous; a constructor
// template cannot deduce from a set of several functions; an explicit
// constructor is no candidate for copy-initialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:5 error "conversion from '<overloaded function type>' to 'Q' is ambiguous"
//@ diag: 20:19 note "candidate constructor"
//@ diag: 20:50 note "candidate constructor"
//@ diag: 25:5 error "no viable conversion from '<overloaded function type>' to 'T'"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) not viable: no overload of 'f' matching 'const T &' for 1st argument"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) not viable: no overload of 'f' matching 'T &&' for 1st argument"
//@ diag: 21:38 note "candidate template ignored: couldn't infer template argument 'F'"
//@ diag: 26:5 error "no viable conversion from '<overloaded function type>' to 'E'"
//@ diag: 22:8 note "candidate constructor (the implicit copy constructor) not viable: no overload of 'f' matching 'const E &' for 1st argument"
//@ diag: 22:8 note "candidate constructor (the implicit move constructor) not viable: no overload of 'f' matching 'E &&' for 1st argument"
//@ diag: 22:28 note "explicit constructor is not a candidate"
int f() { return 1; }
int f(int) { return 2; }
struct Q { int v; Q(int (*p)()) : v(10 + p()) {} Q(int (*p)(int)) : v(20 + p(0)) {} };
struct T { int v; template <class F> T(F* p) : v(30) {} };
struct E { int v; explicit E(int (*p)()) : v(p()) {} };
void use() {
  Q q = f;
  T t = f;
  E e = f;
}
