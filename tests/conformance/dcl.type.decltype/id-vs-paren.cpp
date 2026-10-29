// [dcl.type.decltype]/1.3, 1.5: an unparenthesized id-expression gives the
// entity's declared type; a parenthesized lvalue gives `T&`.
//@ strict-diags
int i;
const int ci = 1;
int &ri = i;
int &&xv();
int &&rri = xv();
struct S { int m; const int cm = 0; mutable int mm; };
S s;
const S cs{};
static_assert(__is_same(decltype(i), int) && __is_same(decltype((i)), int &));
static_assert(__is_same(decltype(ci), const int) && __is_same(decltype((ci)), const int &));
static_assert(__is_same(decltype(ri), int &) && __is_same(decltype((ri)), int &));
static_assert(__is_same(decltype(rri), int &&) && __is_same(decltype((rri)), int &));
static_assert(__is_same(decltype(s.m), int) && __is_same(decltype((s.m)), int &));
static_assert(__is_same(decltype(cs.m), int) && __is_same(decltype((cs.m)), const int &));
static_assert(__is_same(decltype(s.cm), const int) && __is_same(decltype((cs.mm)), int &));
