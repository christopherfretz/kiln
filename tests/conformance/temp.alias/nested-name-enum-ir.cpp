// [expr.prim.id.qual]/1, [temp.alias]/2: an alias template
// specialization naming an enumeration may precede `::`, and names its
// enumerators, also from a template and through a dependent alias
// (WP-1739: kiln left the id-expression erroneous without a diagnostic,
// and code generation stopped on it).
//@ expect: ir
//@ strict-diags
struct C { enum E { e = 2 }; };
enum class G { g = 3 };
template<typename> using EA = C::E;
template<typename> using GA = G;
template<class X> struct T { enum E { e = 4 }; };
template<class X> using TE = typename T<X>::E;
template<class X> int f() { return (int)GA<X>::g + TE<X>::e; }
int h() { return EA<int>::e + (int)GA<int>::g + TE<int>::e + f<char>(); }
