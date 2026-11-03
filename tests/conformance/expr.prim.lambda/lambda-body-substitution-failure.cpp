// [expr.prim.lambda.closure]/2, [temp.deduct.general]/9 (CWG2672): a
// lambda body that fails to substitute fails the lambda-expression itself
// (clang `TransformLambdaExpr` returns an error): no closure with a
// `void` call operator cascades into its initializer or return, an
// expression statement holding it invalidates the enclosing specialization
// (a later call is a substitution failure) while a variable's initializer
// does not, and the body is never in the immediate context, so the error
// is hard even inside a return type (spec/sema-templates.md §5.5, §10.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:23 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 31:14 note "while substituting into a lambda expression here"
//@ diag: 30:27 note "in instantiation of default member initializer 'D<int>::m' requested here"
//@ diag: 33:8 note "in evaluation of exception specification for 'D<int>::D' needed here"
//@ diag: 38:44 error "member reference base type 'int' is not a structure or union"
//@ diag: 38:41 note "while substituting into a lambda expression here"
//@ diag: 39:16 note "in instantiation of function template specialization 'stmt<int>' requested here"
//@ diag: 40:17 error "no matching function for call to 'stmt'"
//@ diag: 38:25 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 42:57 error "member reference base type 'int' is not a structure or union"
//@ diag: 42:54 note "while substituting into a lambda expression here"
//@ diag: 43:20 note "in instantiation of function template specialization 'declinit<int>' requested here"
//@ diag: 46:48 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 46:46 note "while substituting into a lambda expression here"
//@ diag: 48:14 note "while substituting deduced template arguments into function template 'sf' [with T = int]"
//@ diag: 35:56 error "member reference base type 'int' is not a structure or union"
//@ diag: 35:46 note "while substituting into a lambda expression here"
//@ diag: 36:15 note "in instantiation of function template specialization 'ret<int>' requested here"

template <class T> struct D {
  int m = [] { return T::value; }();
};
D<int> d;

template <class T> int ret(T t) { return [&] { return t.foo(); }(); }
int use_ret = ret(1);

template <class T> auto stmt(T t) { [&] { t.foo(); }(); return 1; }
int use_stmt = stmt(1);
int use_stmt2 = stmt(1);

template <class T> auto declinit(T t) { auto l = [&] { t.foo(); }; return 1; }
int use_declinit = declinit(1);
int use_declinit2 = declinit(1);

template <class T> auto sf(T) -> decltype([] { T::nope(); }, 0) { return 0; }
int sf(...) { return 1; }
int use_sf = sf(1);
