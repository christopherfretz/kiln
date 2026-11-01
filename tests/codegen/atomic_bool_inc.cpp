//@ args: -Wno-increment-bool
// WP-165: ++ on _Atomic(bool) (and plain bool), accepted with
// -Wno-increment-bool as in clang's EmitScalarPrePostIncDec.
_Atomic(bool) ab;
bool b;
void f() { ab++; ++ab; b++; ++b; }
bool g() { return ab++; }
bool h() { return ++ab; }
bool i() { return b++; }
bool j() { return ++b; }
bool k(volatile _Atomic(bool) *p) { return (*p)++; }
bool l(volatile _Atomic(bool) *p) { return ++*p; }
int m() { return ab++ + 1; }
