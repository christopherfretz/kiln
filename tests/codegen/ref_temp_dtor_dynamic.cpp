// [class.temporary]/6, [basic.start.static]/2, [expr.const]/14: a
// lifetime-extended temporary with a non-trivial destructor is not a
// constant result, so its extending variable is dynamically initialized
// and the temporary's destructor registered.
struct D { int v; constexpr ~D() {} };
const D& d4 = {6};
struct RD { const D& r; };
RD rd{{1}};
int use() { return d4.v + rd.r.v; }
