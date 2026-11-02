// [temp.deduct.funcaddr]/1, [over.over]/3, [conv.fctptr]/1: deduction from
// the address of a function template ignores a non-deduced exception
// specification, so a potentially-throwing specialization is selected for a
// noexcept target and the conversion that follows fails: "different
// exception specifications" for an initialization (the specialization's
// type printed with its written noexcept-specifier), "assigning to ... from
// incompatible type '<overloaded function type>'" with every candidate
// noted for an assignment ([expr.assign]/3), and "no overload of 't'
// matching" for a call argument (a template-id included). A non-template
// still needs an exact match or a function pointer conversion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:8 error "cannot initialize a variable of type 'void (*)(int) noexcept' with an rvalue of type 'void (*)(int)': different exception specifications"
//@ diag: 51:8 error "cannot initialize a variable of type 'void (*)(long) noexcept' with an rvalue of type 'void (*)(long)': different exception specifications"
//@ diag: 52:8 error "cannot initialize a variable of type 'void (*)(long) noexcept' with an lvalue of type 'void (long)': different exception specifications"
//@ diag: 53:29 error "address of overloaded function 't' does not match required type 'int (long) noexcept'"
//@ diag: 49:25 note "candidate template ignored: could not match 'void' against 'int'"
//@ diag: 55:8 error "cannot initialize a variable of type 'void (*)(int) noexcept' with an lvalue of type 'void (*)(int)': different exception specifications"
//@ diag: 57:15 error "no matching function for call to 'o'"
//@ diag: 56:6 note "candidate function not viable: no known conversion from 'void (*)(int)' to 'void (*)(int) noexcept' for 1st argument"
//@ diag: 57:23 error "no matching function for call to 'o'"
//@ diag: 56:6 note "candidate function not viable: no overload of 't' matching 'void (*)(int) noexcept' for 1st argument"
//@ diag: 57:35 error "no matching function for call to 'o'"
//@ diag: 56:6 note "candidate function not viable: no overload of 't' matching 'void (*)(int) noexcept' for 1st argument"
//@ diag: 60:7 error "cannot initialize a variable of type 'int (*)(int) noexcept' with an rvalue of type 'int (*)(int) noexcept(sizeof(int) > 4)': different exception specifications"
//@ diag: 63:10 error "cannot initialize a variable of type 'int (M::*)(long) noexcept' with an rvalue of type 'int (M::*)(long)': different exception specifications"
//@ diag: 73:7 error "assigning to 'int (*)(long) noexcept' from incompatible type '<overloaded function type>'"
//@ diag: 66:5 note "candidate function has different number of parameters (expected 1 but has 2)"
//@ diag: 65:24 note "candidate function template"
//@ diag: 74:7 error "assigning to 'int (*)(long) noexcept' from incompatible type '<overloaded function type>'"
//@ diag: 65:24 note "candidate function template"
//@ diag: 76:7 error "assigning to 'void (*)(int)' from incompatible type '<overloaded function type>'"
//@ diag: 68:5 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 67:5 note "candidate function has different return type ('void' expected but has 'int')"
//@ diag: 78:7 error "assigning to 'int (*)(int)' from incompatible type '<overloaded function type>'"
//@ diag: 70:6 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'char')"
//@ diag: 69:6 note "candidate function has different return type ('int' expected but has 'void')"
//@ diag: 80:7 error "assigning to 'void (*)(int) noexcept' from incompatible type '<overloaded function type>'"
//@ diag: 70:6 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'char')"
//@ diag: 69:6 note "candidate function has different exception specification"
//@ diag: 82:7 error "assigning to 'int (*)(short, int)' from incompatible type '<overloaded function type>'"
//@ diag: 66:5 note "candidate function has type mismatch at 1st parameter (expected 'short' but has 'char')"
//@ diag: 65:24 note "candidate function template"
//@ diag: 84:7 error "assigning to 'int' from incompatible type '<overloaded function type>'"
//@ diag: 66:5 note "candidate function"
//@ diag: 65:24 note "candidate function template"

void t1(int);
template <class T> void t(T);
void (*p1)(int) noexcept = &t1;
void (*p2)(long) noexcept = &t;
void (*p3)(long) noexcept = t;
int (*p4)(long) noexcept = &t;
void (*q)(int);
void (*p5)(int) noexcept = q;
void o(void (*)(int) noexcept);
void call() { o(&t1); o(&t<int>); o(&t); }

template <class T> int u(T) noexcept(sizeof(T) > 4) { return 0; }
int (*p6)(int) noexcept = &u;
int (*p7)(long) noexcept = &u;
struct M { template <class T> int m(T); };
int (M::*pm)(long) noexcept = &M::m;

template <class T> int w(T);
int w(char, char);
int g(int);
int g(long);
void h(int);
void h(char);
void assign() {
  int (*a)(long) noexcept;
  a = &w;
  a = w<long>;
  void (*b)(int);
  b = &g;
  int (*c)(int);
  c = h;
  void (*d)(int) noexcept;
  d = h;
  int (*e)(short, int);
  e = &w;
  int x;
  x = &w;
}
