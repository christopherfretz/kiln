// [expr.const]/10, [temp.inst]/7: a constant-evaluation note naming a
// variable template specialization names it with its template arguments
// (`v<int>`, clang `VarTemplateSpecializationDecl::getNameForDiagnostic`),
// canonical and without trailing default arguments (`z<char>`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 20:19 note "read of non-const variable 'v<int>' is not allowed in a constant expression"
//@ diag: 19:24 note "declared here"
//@ diag: 23:15 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 23:19 note "initializer of 'w<A>' is not a constant expression"
//@ diag: 21:30 note "declared here"
//@ diag: 25:15 error "constexpr variable 'zz' must be initialized by a constant expression"
//@ diag: 25:20 note "initializer of 'z<char>' is not a constant expression"
//@ diag: 24:45 note "declared here"
//@ diag: 27:15 error "constexpr variable 'yy' must be initialized by a constant expression"
//@ diag: 27:20 note "initializer of 'w<A>' is not a constant expression"
//@ diag: 21:30 note "declared here"
template <class T> int v = 1;
constexpr int x = v<int>;
template <class T> const int w = T::nope + 0;
struct A { static const int nope; };
constexpr int y = w<A>;
template <class T, class U = int> const int z = sizeof(T) + v<T>;
constexpr int zz = z<char>;
using I = int;
constexpr int yy = w<A> + v<I>;
