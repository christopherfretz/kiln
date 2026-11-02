// [temp.deduct.general]/8, /11.1, /11.10: each invalid substituted parameter
// type is a substitution failure, reported in the candidate note with the
// diagnostic the substitution produced (clang `SubstParmVarDecl`,
// `BuildArrayType`, `BuildFunctionType`, `BuildPointerType`). Outside a
// SFINAE context (a member of a class template specialization) a `void`
// parameter is an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:39 error "argument may not have 'void' type"
//@ diag: 28:9 note "in instantiation of template class 'S<void>' requested here"
//@ diag: 27:56 error "argument may not have 'void' type"
//@ diag: 37:3 error "no matching function for call to 'byval'"
//@ diag: 29:25 note "candidate template ignored: substitution failure [with T = void]: argument may not have 'void' type"
//@ diag: 38:3 error "no matching function for call to 'deduced'"
//@ diag: 30:25 note "candidate template ignored: substitution failure [with T = void]: argument may not have 'void' type"
//@ diag: 39:3 error "no matching function for call to 'pack'"
//@ diag: 31:28 note "candidate template ignored: substitution failure [with T = <int, void>]: argument may not have 'void' type"
//@ diag: 40:3 error "no matching function for call to 'fnptr'"
//@ diag: 32:25 note "candidate template ignored: substitution failure [with T = void]: argument may not have 'void' type"
//@ diag: 41:3 error "no matching function for call to 'arr'"
//@ diag: 33:25 note "candidate template ignored: substitution failure [with T = void]: array has incomplete element type 'void'"
//@ diag: 42:3 error "no matching function for call to 'abs_arr'"
//@ diag: 34:25 note "candidate template ignored: substitution failure [with T = A]: array of abstract class type 'A'"
//@ diag: 43:3 error "no matching function for call to 'arr_void'"
//@ diag: 35:25 note "candidate template ignored: substitution failure [with T = void]: array has incomplete element type 'void'"
struct A { virtual void f() = 0; };
template <class T> struct S { void f(T); void g(int, T x); };
S<void> s;
template <class T> void byval(T);
template <class T> void deduced(T);
template <class... T> void pack(T...);
template <class T> void fnptr(void (*)(T));
template <class T> void arr(T[3]);
template <class T> void abs_arr(T (*)[2]);
template <class T> void arr_void(T (*)[4]);
void test() {
  byval<void>(0);
  deduced(void());
  pack<int, void>(0, 0);
  fnptr<void>(nullptr);
  arr<void>(nullptr);
  abs_arr<A>(nullptr);
  arr_void<void>(nullptr);
}
