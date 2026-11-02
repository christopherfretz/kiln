// [expr.prim.req.general]/4, [dcl.spec.auto.general]/5: a placeholder in a
// requires-expression parameter (outside a trailing return type) is
// "'auto' not allowed in requires expression parameter" at the placeholder;
// the parameter recovers as an error type, so its uses do not cascade.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:41 error "'auto' not allowed in requires expression parameter"
//@ diag: 12:52 error "'auto' not allowed in requires expression parameter"
//@ diag: 13:21 error "'auto' not allowed in requires expression parameter"
//@ diag: 15:21 error "'decltype(auto)' not allowed in requires expression parameter"
template<class T> concept A = requires (auto x) { x + 1; };
template<class T> concept B = requires (T t, const auto &y) { t; y.size(); };
bool b1 = requires (auto x, int y) { x * y; };
bool b2 = requires (auto (*p)(int) -> int) { p(1); };
bool b3 = requires (decltype(auto) z) { z; };
