// [expr.pre]/4, [expr.mul]/4, [expr.shift]/1, [expr.const]/5.8 (C23 6.2.5):
// a wide `_BitInt(N)` (N > 128, evaluated by the Oven's limb arithmetic,
// spec/oven.md "`_BitInt`") whose signed result is out of range (`+`, `-`,
// `*`, negation, `++`/`--`, `MIN / -1` and `MIN % -1`), a division by zero, a
// shift count out of range, a read of an uninitialized object and a write to
// a const one are not core constant expressions; unsigned arithmetic wraps.
// Notes print wide argument values (also inside a class) in decimal.
// clang is the oracle; x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is
// 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 62:13 error "constexpr variable 'ov1' must be initialized by a constant expression"
//@ diag: 62:22 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 63:13 error "constexpr variable 'ov2' must be initialized by a constant expression"
//@ diag: 63:32 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 64:13 error "constexpr variable 'ov3' must be initialized by a constant expression"
//@ diag: 64:19 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 65:13 error "constexpr variable 'ov4' must be initialized by a constant expression"
//@ diag: 65:22 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 66:13 error "constexpr variable 'ov5' must be initialized by a constant expression"
//@ diag: 66:22 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 67:13 error "constexpr variable 'dz1' must be initialized by a constant expression"
//@ diag: 67:24 note "division by zero"
//@ diag: 68:13 error "constexpr variable 'dz2' must be initialized by a constant expression"
//@ diag: 68:24 note "division by zero"
//@ diag: 69:13 error "constexpr variable 'sh1' must be initialized by a constant expression"
//@ diag: 69:23 note "shift count 200 >= width of type 'W' (aka '_BitInt(200)') (200 bits)"
//@ diag: 70:13 error "constexpr variable 'sh2' must be initialized by a constant expression"
//@ diag: 70:23 note "negative shift count -1"
//@ diag: 71:13 error "constexpr variable 'sh3' must be initialized by a constant expression"
//@ diag: 71:23 note "negative shift count -3"
//@ diag: 73:13 error "constexpr variable 'ov6' must be initialized by a constant expression"
//@ diag: 72:34 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 73:19 note "in call to 'inc1()'"
//@ diag: 75:13 error "constexpr variable 'ov7' must be initialized by a constant expression"
//@ diag: 74:32 note "value -803469022129495137770981046170581301261101496891396417650689 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 75:19 note "in call to 'inc2()'"
//@ diag: 77:13 error "constexpr variable 'ov8' must be initialized by a constant expression"
//@ diag: 76:33 note "value 803469022129495137770981046170581301261101496891396417650688 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 77:19 note "in call to 'inc3()'"
//@ diag: 79:13 error "constexpr variable 'un' must be initialized by a constant expression"
//@ diag: 78:33 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 79:18 note "in call to 'un1()'"
//@ diag: 81:13 error "constexpr variable 'cwv' must be initialized by a constant expression"
//@ diag: 80:77 note "modification of object of const-qualified type 'const _BitInt(200)' is not allowed in a constant expression"
//@ diag: 81:19 note "in call to 'cw()'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:19 note "value -803469022129495137770981046170581301261101496891396417650689 is outside the range of representable values of type 'W' (aka '_BitInt(200)')"
//@ diag: 87:15 error "static assertion expression is not an integral constant expression"
//@ diag: 86:37 note "division by zero"
//@ diag: 87:15 note "in call to 'fz(1427247692705959881058285969449495136382746624, 0)'"
//@ diag: 90:15 error "static assertion expression is not an integral constant expression"
//@ diag: 89:35 note "division by zero"
//@ diag: 90:15 note "in call to 'gz({-7, 1})'"
using W = _BitInt(200);
using U = unsigned _BitInt(256);
constexpr W one = 1;
constexpr W mx = ((one << 198) - 1) * 2 + 1;
constexpr W mn = -mx - 1;
static_assert(mx + mn == -1 && (U)mn == ~((U(1) << 199) - 1));
constexpr W ov1 = mx + 1;
constexpr W ov2 = (one << 198) * 2;
constexpr W ov3 = -mn;
constexpr W ov4 = mn / -1;
constexpr W ov5 = mn % -1;
constexpr W dz1 = W(1) / 0;
constexpr W dz2 = W(1) % W(0);
constexpr W sh1 = one << 200;
constexpr W sh2 = one << -1;
constexpr W sh3 = one >> W(-3);
constexpr W inc1() { W x = mx; x += 1; return x; }
constexpr W ov6 = inc1();
constexpr W inc2() { W x = mn; --x; return x; }
constexpr W ov7 = inc2();
constexpr W inc3() { W x = mx; x++; return x; }
constexpr W ov8 = inc3();
constexpr W un1() { W x; return x + 1; }
constexpr W un = un1();
constexpr W cw() { const _BitInt(200) x = 1; W *p = const_cast<W *>(&x); *p = 2; return *p; }
constexpr W cwv = cw();
constexpr U uw = U(0) - 1;
static_assert(uw + 1 == 0 && uw == ~U(0) && uw * uw == 1);
static_assert(mx > 0, "unreachable");
static_assert(-mx - 2 < 0);
constexpr W fz(W a, W z) { return a / z; }
static_assert(fz(one << 150, 0) == 0);
struct SW { W x; int y; };
constexpr int gz(SW s) { return 1 / (s.y - 1); }
static_assert(gz(SW{-7, 1}) == 0);
