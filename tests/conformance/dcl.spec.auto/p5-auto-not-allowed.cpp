// [dcl.spec.auto.general]/5-6, [dcl.type.class.deduct], [expr.prim.req.general]/4,
// [temp.param]/9: a placeholder type outside the positions that permit one is
// "'auto' not allowed in <position>" (clang's wording and selector) at the
// placeholder; with a trailing return type the placeholder checked is the
// one inside it. Recovery is `int` / an error type, so nothing cascades.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 38:11 error "'auto' not allowed in type alias"
//@ diag: 39:11 error "'auto' not allowed in type alias"
//@ diag: 40:12 error "'decltype(auto)' not allowed in type alias"
//@ diag: 41:9 error "'auto' not allowed in typedef"
//@ diag: 42:9 error "'auto' not allowed in typedef"
//@ diag: 43:16 error "'auto' not allowed here"
//@ diag: 44:17 error "'auto' not allowed here"
//@ diag: 45:17 error "'decltype(auto)' not allowed here"
//@ diag: 46:3 error "'auto' not allowed in template argument"
//@ diag: 47:3 error "'auto' not allowed in template argument"
//@ diag: 48:8 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 49:23 error "'auto' not allowed here"
//@ diag: 50:10 error "'auto' not allowed here"
//@ diag: 51:21 error "'auto' not allowed here"
//@ diag: 52:12 error "'auto' not allowed in non-static struct member"
//@ diag: 52:20 error "'auto' not allowed in non-static struct member"
//@ diag: 53:11 error "'auto' not allowed in non-static union member"
//@ diag: 54:11 error "'auto' not allowed in non-static class member"
//@ diag: 55:26 error "'auto' not allowed in exception declaration"
//@ diag: 56:20 error "'auto' not allowed here"
//@ diag: 57:20 error "'auto' not allowed in template argument"
//@ diag: 59:21 error "'auto' not allowed in requires expression parameter"
//@ diag: 60:42 error "'auto' not allowed in requires expression parameter"
//@ diag: 61:25 error "'auto' not allowed here"
//@ diag: 62:11 error "'auto' not allowed here"
//@ diag: 63:9 error "'decltype(auto)' not allowed in function prototype"
//@ diag: 64:14 error "'decltype(auto)' not allowed in lambda parameter"
namespace std { class type_info; }
template<class T> struct S {};
using F = auto(*)(int)->auto;
using G = auto;
using DA = decltype(auto);
typedef auto H;
typedef auto (*HP)(int);
int a = sizeof(auto);
int b = alignof(auto);
int dk = sizeof(decltype(auto));
S<auto> s;
S<auto(*)(int)> s2;
S<void(auto)> s3;
void p(int x = sizeof(auto));
int c = (auto)1;
int d = static_cast<auto>(1);
struct M { auto m; auto n = 1; };
union U { auto u; };
class K { auto k; };
void q() { try {} catch (auto) {} }
int r = __typeof__(auto)(1);
template<class T = auto> struct D {};
template<auto (*fp)(int) -> auto> struct E {};
bool rq = requires (auto x) { x; };
template<class T> concept CC = requires (auto x) { x; };
void t() { (void)typeid(auto); }
enum En : auto {};
void pf(decltype(auto) x);
auto lm = [](decltype(auto) x) { return 0; };
