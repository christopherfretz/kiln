// [temp.param]/7: a non-type template parameter of class type must be a
// literal type; clang explains why the type is not literal.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:13 error "non-type template parameter has non-literal type 'E'"
//@ diag: 15:8 note "'E' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 20:13 error "non-type template parameter has non-literal type 'L'"
//@ diag: 16:19 note "'L' is not literal because its destructor is not constexpr"
//@ diag: 21:13 error "non-type template parameter has non-literal type 'M'"
//@ diag: 17:14 note "'M' is not literal because it has data member 'e' of non-literal type 'E'"
//@ diag: 22:13 error "non-type template parameter has non-literal type 'N'"
//@ diag: 18:12 note "'N' is not literal because it has base class 'L' of non-literal type"
//@ diag: 23:13 error "non-type template parameter has non-literal type 'E'"
//@ diag: 15:8 note "'E' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
struct E { E(); int x; };
struct L { int x; ~L(); };
struct M { E e; };
struct N : L {};
template <E e> struct TE {};
template <L l> struct TL {};
template <M m> struct TM {};
template <N n> struct TN {};
template <E e> void fe();
