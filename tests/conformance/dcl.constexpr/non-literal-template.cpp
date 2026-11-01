// [dcl.constexpr]/6, [temp.inst]/2: checking that a constexpr variable
// has a literal type requires its type to be complete, which
// instantiates a class template specialization first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:13 error "constexpr variable cannot have non-literal type 'const N'"
//@ diag: 10:8 note "'N' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 15:17 error "constexpr variable cannot have non-literal type 'const TT<N>'"
//@ diag: 14:33 note "'TT<N>' is not literal because it has data member 't' of non-literal type 'N'"
struct N { N(); ~N(); int v; };
constexpr N n{};
constexpr int f(N) { return 0; }
constexpr N g() { return N(); }
template<class T> struct TT { T t; };
constexpr TT<N> tt{};
