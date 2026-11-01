// Lifetime-extended temporaries of namespace-scope references (WP-204):
// the `_ZGR` global is `constant` exactly when clang's materialized type
// (the operand of the MaterializeTemporaryExpr past no-op casts and
// subobject adjustments) is a const type without `mutable` members or a
// destructor: a temporary created by converting to the referenced type,
// or a braced list.
struct P { int a, b; };
struct M { mutable int a; };
struct Base { int b; };
struct Der : Base { int d; };
const int& r1 = 5;
const P& r2 = P{1, 2};
const M& r3 = M{1};
int&& r5 = 6;
const int (&r6)[2] = {1, 2};
const double& r7 = 1;
const P& r8 = {3, 4};
constexpr const int& r9 = 7;
P&& r10 = P{5, 6};
const long& r11 = r1;
const M& r12 = {1};
const Base& r13 = Der{{1}, 2};
const int& r14 = P{7, 8}.a;
const int& r15 = (8);
const P& r16 = (P{9, 9});
