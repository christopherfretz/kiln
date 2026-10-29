//@ expect: ast
// [expr.eq]/4: member pointers compare equal or unequal after conversion to
// their composite pointer type ([expr.type]/4): a null pointer constant
// becomes a null member pointer ([conv.mem]/1, NullToMemberPointer) and a
// pointer to member of a base becomes one of the derived class ([conv.mem]/2,
// BaseToDerivedMemberPointer). The result is bool.
struct B { int x; int w; void f(); void g(); };
struct D : B { int y; };
struct E : D {}; // references D for clang too (kiln marks a nested-name-specifier's class referenced)
int B::*pb = &B::x;
int B::*qb = &B::w;
int D::*pd = &D::y;
void (B::*pf)() = &B::f;
bool a1 = pb == qb;
bool a2 = pd != pb;
bool a3 = pb == pd;
bool a4 = pb == nullptr;
bool a5 = 0 == pb;
bool a6 = pf != nullptr;
bool a7 = pf == &B::g;
bool a8 = &B::x != pb;
bool a9 = pb != 0L;
const int B::*cb = nullptr;
bool a10 = cb == pb;
