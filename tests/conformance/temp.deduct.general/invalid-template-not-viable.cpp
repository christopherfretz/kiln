// [temp.deduct.general]/8, [over.match.viable], [temp.expl.spec]: a function
// template declared with a parameter type that names no type (`S::nope`, an
// undeclared `undeclared_t`) is invalid. Deduction against it fails (clang's
// `TemplateDeductionResult::Invalid`), so a call only it could match is "no
// matching function" with no candidate note; beside other declarations
// lookup ignores it (clang's `LookupResult::resolveKind`), so `t3` names the
// valid function. A non-template function with an invalid parameter
// recovers that parameter to `int` and stays callable. An explicit
// specialization that is itself invalid matches no template and reports
// nothing more; a valid one of an invalid template finds no match, unnoted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:34 error "no type named 'nope' in 'S'"
//@ diag: 27:28 error "unknown type name 'undeclared_t'"
//@ diag: 28:12 error "no type named 'nope2' in 'S'"
//@ diag: 29:30 error "unknown type name 'bad_t'"
//@ diag: 31:31 error "unknown type name 'missing_t'"
//@ diag: 32:26 error "unknown type name 'missing_t'"
//@ diag: 33:31 error "unknown type name 'missing2_t'"
//@ diag: 34:18 error "no function template matches function template specialization 't5'"
//@ diag: 36:3 error "no matching function for call to 't1'"
//@ diag: 37:3 error "no matching function for call to 't2'"
//@ diag: 38:3 error "no matching function for call to 't2'"
//@ diag: 42:10 error "cannot initialize a variable of type 'void (*)(int, int)' with an lvalue of type 'int (int, long)': type mismatch at 2nd parameter ('int' vs 'long')"
struct S {};
template <class T> void t1(T, S::nope);
template <class T> void t2(undeclared_t, T);
void n1(S::nope2);
template <class T> int t3(T, bad_t) { return 0; }
int t3(int, long);
template <class T> void t4(T, missing_t);
template <> void t4(int, missing_t);
template <class T> void t5(T, missing2_t);
template <> void t5(int, int);
void use() {
  t1(1, 2);
  t2(1, 2);
  t2<int>(1, 2);
  n1(1);
  t3(1, 2);
  t3(1.0, 2);
  void (*p)(int, int) = t3;
}
