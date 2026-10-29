// [over.match.ctor]: no viable, ambiguous, deleted and explicit
// constructors in class initialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:5 error "call to constructor of 'S' is ambiguous"
//@ diag: 31:3 note "candidate constructor"
//@ diag: 32:3 note "candidate constructor"
//@ diag: 33:3 note "candidate constructor has been explicitly deleted"
//@ diag: 34:12 note "candidate constructor"
//@ diag: 38:5 error "call to deleted constructor of 'S'"
//@ diag: 33:3 note "'S' has been explicitly marked deleted here"
//@ diag: 39:5 error "conversion from 'double' to 'S' is ambiguous"
//@ diag: 31:3 note "candidate constructor"
//@ diag: 32:3 note "candidate constructor"
//@ diag: 33:3 note "candidate constructor has been explicitly deleted"
//@ diag: 40:5 error "no matching constructor for initialization of 'S'"
//@ diag: 30:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const char[2]' to 'const S' for 1st argument"
//@ diag: 30:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const char[2]' to 'S' for 1st argument"
//@ diag: 31:3 note "candidate constructor not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 32:3 note "candidate constructor not viable: no known conversion from 'const char[2]' to 'long' for 1st argument"
//@ diag: 33:3 note "candidate constructor not viable: no known conversion from 'const char[2]' to 'char' for 1st argument"
//@ diag: 34:12 note "candidate constructor not viable: no known conversion from 'const char[2]' to 'double' for 1st argument"
//@ diag: 41:5 error "no matching constructor for initialization of 'S'"
//@ diag: 31:3 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 32:3 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 33:3 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 34:12 note "candidate constructor not viable: requires 1 argument, but 2 were provided"
//@ diag: 30:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 30:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
struct S {
  S(int);
  S(long);
  S(char) = delete;
  explicit S(double);
};
void t() {
  S a(1u);
  S b('c');
  S c = 1.0;
  S d("x");
  S e(1, 2);
}
