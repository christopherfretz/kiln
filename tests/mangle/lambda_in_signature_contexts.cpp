// Mangling differential: a closure in a function template's signature,
// numbered by the resolutions of an uncalled template-id `f<int>` in each
// context that converts it ([over.over]/1, [temp.deduct.funcaddr],
// [expr.prim.lambda.closure]/2; spec/sema-lambda.md §3.5, WP-1170). Each
// resolution substitutes the explicit arguments into the signature once,
// and forming the specialization substitutes it once more, so the
// `f<long>` formed after the use shows how many numbers the context took.
// `aN` defaults `U = void`, so `aN<int>` alone names `aN<int, void>`;
// `bN` has no default and only a target deduces `U`. One template per
// context. clang 19 and 22 agree on every name here.
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
using FP = int (*)(int *, int);
using FR = int (&)(int *, int);
using FPV = int (*)(void *, int);
using FRV = int (&)(void *, int);
void take(FP);
void takeR(FR);
template <class F> void tdeduce(F);
template <class T, class U = void> auto a0(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a0() { (void)a0<int>; }
template <class T, class U = void> auto a1(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a1() { FPV q = a1<int>; }
template <class T, class U = void> auto a2(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a2() { FPV q{a2<int>}; }
template <class T, class U = void> auto a3(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a3() { FP q = a3<int>; }
template <class T, class U> auto b3(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b3() { FP q = b3<int>; }
template <class T, class U = void> auto a4(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a4() { FP q{a4<int>}; }
template <class T, class U> auto b4(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b4() { FP q{b4<int>}; }
template <class T, class U = void> auto a5(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a5() { FP q(a5<int>); }
template <class T, class U> auto b5(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b5() { FP q(b5<int>); }
template <class T, class U = void> auto a6(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a6() { FP q = &a6<int>; }
template <class T, class U> auto b6(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b6() { FP q = &b6<int>; }
template <class T, class U = void> auto a7(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a7() { FP q{&a7<int>}; }
template <class T, class U> auto b7(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b7() { FP q{&b7<int>}; }
template <class T, class U = void> auto a8(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a8() { FP q; q = a8<int>; }
template <class T, class U> auto b8(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b8() { FP q; q = b8<int>; }
template <class T, class U = void> auto a9(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a9() { take(a9<int>); }
template <class T, class U> auto b9(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b9() { take(b9<int>); }
template <class T, class U = void> auto a10(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a10() { ::take(a10<int>); }
template <class T, class U> auto b10(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b10() { ::take(b10<int>); }
template <class T, class U = void> auto a11(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a11() { FR r = a11<int>; }
template <class T, class U> auto b11(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b11() { FR r = b11<int>; }
template <class T, class U = void> auto a12(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a12() { FR r{a12<int>}; }
template <class T, class U> auto b12(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b12() { FR r{b12<int>}; }
template <class T, class U = void> auto a13(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a13() { takeR(a13<int>); }
template <class T, class U> auto b13(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b13() { takeR(b13<int>); }
template <class T, class U = void> auto a14(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a14() { auto q = static_cast<FP>(a14<int>); }
template <class T, class U> auto b14(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b14() { auto q = static_cast<FP>(b14<int>); }
template <class T, class U = void> auto a15(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a15() { auto q = (FP)a15<int>; }
template <class T, class U> auto b15(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b15() { auto q = (FP)b15<int>; }
template <class T, class U = void> auto a16(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a16() { auto q = FP(a16<int>); }
template <class T, class U> auto b16(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b16() { auto q = FP(b16<int>); }
template <class T, class U = void> auto a17(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a17() { tdeduce<FP>(a17<int>); }
template <class T, class U> auto b17(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b17() { tdeduce<FP>(b17<int>); }
template <class T, class U = void> auto a18(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a18() { tdeduce<FPV>(a18<int>); }
template <class T, class U = void> auto a19(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a19() { tdeduce(a19<int>); }
template <class T, class U = void> auto a20(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a20() { tdeduce(&a20<int>); }
template <class T, class U = void> auto a21(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a21() { tdeduce<FP>(&a21<int>); }
template <class T, class U> auto b21(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b21() { tdeduce<FP>(&b21<int>); }
template <class T, class U = void> auto a22(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a22() { FRV r{a22<int>}; }
template <class T, class U = void> auto a23(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a23() { FRV r = a23<int>; }
template <class T, class U = void> auto a24(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a24() { const FP &q = a24<int>; }
template <class T, class U> auto b24(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b24() { const FP &q = b24<int>; }
template <class T, class U = void> auto a25(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a25() { FP q[1] = {a25<int>}; }
template <class T, class U> auto b25(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b25() { FP q[1] = {b25<int>}; }
template <class T, class U = void> auto a26(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a26() { struct SS { FP p; } s{a26<int>}; }
template <class T, class U> auto b26(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b26() { struct SS { FP p; } s{b26<int>}; }
template <class T, class U = void> auto a27(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a27() { FP q = (a27<int>); }
template <class T, class U> auto b27(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b27() { FP q = (b27<int>); }
template <class T, class U = void> auto a28(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a28() { auto q = a28<int>; }
template <class T, class U = void> auto a29(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a29() { auto *q = &a29<int>; }
template <class T, class U = void> auto a30(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a30() { auto &q = a30<int>; }
template <class T, class U = void> auto a31(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a31() { FP q = nullptr; q = &a31<int>; }
template <class T, class U> auto b31(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b31() { FP q = nullptr; q = &b31<int>; }
template <class T, class U = void> auto a32(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a32() { FPV q = nullptr; q = a32<int>; }
template <class T, class U = void> auto a33(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_a33() { FR r(a33<int>); }
template <class T, class U> auto b33(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
void use_b33() { FR r(b33<int>); }
int main() {
  return a0<long>((int *)nullptr, 1)() +
         a1<long>((int *)nullptr, 1)() +
         a2<long>((int *)nullptr, 1)() +
         a3<long>((int *)nullptr, 1)() +
         b3<long>((int *)nullptr, 1)() +
         a4<long>((int *)nullptr, 1)() +
         b4<long>((int *)nullptr, 1)() +
         a5<long>((int *)nullptr, 1)() +
         b5<long>((int *)nullptr, 1)() +
         a6<long>((int *)nullptr, 1)() +
         b6<long>((int *)nullptr, 1)() +
         a7<long>((int *)nullptr, 1)() +
         b7<long>((int *)nullptr, 1)() +
         a8<long>((int *)nullptr, 1)() +
         b8<long>((int *)nullptr, 1)() +
         a9<long>((int *)nullptr, 1)() +
         b9<long>((int *)nullptr, 1)() +
         a10<long>((int *)nullptr, 1)() +
         b10<long>((int *)nullptr, 1)() +
         a11<long>((int *)nullptr, 1)() +
         b11<long>((int *)nullptr, 1)() +
         a12<long>((int *)nullptr, 1)() +
         b12<long>((int *)nullptr, 1)() +
         a13<long>((int *)nullptr, 1)() +
         b13<long>((int *)nullptr, 1)() +
         a14<long>((int *)nullptr, 1)() +
         b14<long>((int *)nullptr, 1)() +
         a15<long>((int *)nullptr, 1)() +
         b15<long>((int *)nullptr, 1)() +
         a16<long>((int *)nullptr, 1)() +
         b16<long>((int *)nullptr, 1)() +
         a17<long>((int *)nullptr, 1)() +
         b17<long>((int *)nullptr, 1)() +
         a18<long>((int *)nullptr, 1)() +
         a19<long>((int *)nullptr, 1)() +
         a20<long>((int *)nullptr, 1)() +
         a21<long>((int *)nullptr, 1)() +
         b21<long>((int *)nullptr, 1)() +
         a22<long>((int *)nullptr, 1)() +
         a23<long>((int *)nullptr, 1)() +
         a24<long>((int *)nullptr, 1)() +
         b24<long>((int *)nullptr, 1)() +
         a25<long>((int *)nullptr, 1)() +
         b25<long>((int *)nullptr, 1)() +
         a26<long>((int *)nullptr, 1)() +
         b26<long>((int *)nullptr, 1)() +
         a27<long>((int *)nullptr, 1)() +
         b27<long>((int *)nullptr, 1)() +
         a28<long>((int *)nullptr, 1)() +
         a29<long>((int *)nullptr, 1)() +
         a30<long>((int *)nullptr, 1)() +
         a31<long>((int *)nullptr, 1)() +
         b31<long>((int *)nullptr, 1)() +
         a32<long>((int *)nullptr, 1)() +
         a33<long>((int *)nullptr, 1)() +
         b33<long>((int *)nullptr, 1)();
}
