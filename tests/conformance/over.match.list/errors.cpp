// [over.match.list]: copy-list-initialization selecting an explicit
// constructor is ill-formed (/1); no viable constructor; narrowing.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:5 error "chosen constructor is explicit in copy-initialization"
//@ diag: 16:12 note "explicit constructor declared here"
//@ diag: 26:5 error "call to constructor of 'T' is ambiguous"
//@ diag: 20:3 note "candidate constructor"
//@ diag: 21:3 note "candidate constructor"
//@ diag: 27:5 error "no matching constructor for initialization of 'T'"
//@ diag: 20:3 note "candidate constructor not viable: requires 2 arguments, but 3 were provided"
//@ diag: 21:3 note "candidate constructor not viable: requires 2 arguments, but 3 were provided"
//@ diag: 19:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 3 were provided"
//@ diag: 19:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 3 were provided"
struct S {
  explicit S(int, int);
  S(double);
};
struct T {
  T(int, int);
  T(long, long);
};
void t() {
  S a = {1, 2};
  S b{1, 2};
  T c{1, 2L};
  T d = {1, 2, 3};
}
