// [temp.inst]/7, [temp.inst]/1: the "in instantiation" note of an error
// in a variable template specialization's declaration (its type) names
// the pattern, as clang does: 'v' and 'u<T *>' for a primary and a
// partial specialization, 'S2::m' for a static member template. So does
// the note of an error in the initializer when the declared type is
// deduced (it is substituted with the declaration). With a declared type
// the initializer is instantiated on its own and the note names the
// specialization ('w<int>'); a member template of a class template
// specialization is named by its specialization ('X<char>::p<int>').
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 26:9 note "in instantiation of variable template specialization 'v' requested here"
//@ diag: 30:32 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 31:9 note "in instantiation of variable template specialization 'u<T *>' requested here"
//@ diag: 33:28 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:10 note "in instantiation of variable template specialization 'u2<T *>' requested here"
//@ diag: 35:57 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 36:14 note "in instantiation of static data member 'S2::m' requested here"
//@ diag: 37:74 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 38:18 note "in instantiation of static data member 'X<char>::p<int>' requested here"
//@ diag: 27:27 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 28:9 note "in instantiation of variable template specialization 'w<int>' requested here"
template<class T> auto v = T::x;
int y = v<int>;
template<class T> int w = T::x;
int z = w<int>;
template<class T> auto u = 0;
template<class T> auto u<T*> = T::x;
int q = u<int*>;
template<class T> int u2 = 0;
template<class T> typename T::x u2<T*>;
int q2 = u2<int*>;
struct S2 { template<class T> static constexpr auto m = T::x; };
int r2 = S2::m<int>;
template<class T> struct X { template<class U> static constexpr auto p = U::x; };
int f = X<char>::p<int>;
