// [dcl.fct.def.general]/8: a predefined identifier lowers to a private
// unnamed_addr string named `<identifier>.<mangled function>` when first
// emitted; an equal string emitted before (or after) is shared, and a
// constant initializer names it `.str` (clang EmitPredefinedLValue).
const char *s() { return "f"; }
const char *f() { return __func__; }
const char *g() { return __FUNCTION__; }
const char *g2() { return __func__; }
struct S { const char *m() const { return __PRETTY_FUNCTION__; } };
const char *h() { return S().m(); }
template <class T> const char *t() { return __PRETTY_FUNCTION__; }
const char *u() { return t<int>(); }
struct TypeID { const char *s; constexpr TypeID(const char *p) : s(p) {} };
const TypeID &id() { static const TypeID x(__PRETTY_FUNCTION__); return x; }
const char *after() { return "g2"; }
