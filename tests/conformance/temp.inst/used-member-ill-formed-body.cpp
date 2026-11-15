// [temp.inst]/11, GNU `used`: implicitly instantiating `S<int>`
// instantiates the definition of its `used` member function, so an
// ill-formed body is diagnosed although nothing calls it (clang
// `VisitCXXMethodDecl` → `MarkFunctionReferenced` at the class's point
// of instantiation, which the note names); the other member's body is
// not instantiated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:44 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 16:8 note "in instantiation of member function 'S<int>::bad' requested here"
template <class T> struct S {
  int ok() { return T::fine; }
  __attribute__((used)) int bad() { return T::nope; }
  int pad;
};
S<int> s;
