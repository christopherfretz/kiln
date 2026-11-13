// [dcl.type.elab], [dcl.typedef]/2: in an alias-declaration's type-id,
// `struct X<A>` followed by the declaration's `;` refers to the
// specialization; it declares nothing, so no `template<>` is needed
// (as in real code: `using Traits = struct TypeTraits<CodeSet>;`).
//@ strict-diags
template <class T> struct TT;
template <> struct TT<int> { static const int v = 1; };
using Y = struct TT<int>;
using Z = struct Fwd;
static_assert(Y::v == 1);
struct Fwd { int a; };
Z z = {2};
