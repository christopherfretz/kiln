// Debug info (WP-1414): the template value arguments in a specialization's
// name as clang's `TemplateArgument::print` / `APValue::printPretty`
// spell them ([temp.arg.nontype]): only the value's class is named, a
// member or base subobject is a braced list (`Out{{2}}`, `D{{1}, 2}`); a
// floating-point value is its `double` in `%e` form (`1.000000e+00`,
// `-0.000000e+00`, `INF`, `nan`); a closure type is `(lambda at F:L:C)`;
// integers of every width and signedness, character types, enumerators,
// pointers, member pointers, packs and empty packs as before. clang 19
// and 22 agree.
//@ args: -g
struct In { int i; };
struct Out { In in; };
struct F { double d; float f; };
struct Base { int b; };
struct D : Base { int d; };
struct Arr { int a[3]; };
union U { int i; float f; };
struct Two { int a; In b; };
struct Empty {};
enum E { e0, e1 };
enum class EC : short { a = -1, b = 7 };
int gi;
int ga[4];
int fn(int);
struct M { int x; int f(); };
template <auto V> struct T { int k = 0; };
template <auto... V> struct P { int k = 0; };
template <class C> struct W { int k = 0; };
constexpr auto lam = [](int) { return 0; };
// Not `const` (a const member makes clang's `isTrivial` false, a
// separate difference).
struct H { __remove_const(decltype(lam)) l; int i; };
T<Out{{2}}> t1;
T<F{1.0, -0.0f}> t2;
T<1.0> t3;
T<-0.0> t4;
T<1.5f> t5;
T<__builtin_inf()> t6;
T<-__builtin_inf()> t7;
T<__builtin_nan("")> t8;
T<(float)0.1> t9;
T<123456789.0> t10;
T<1e-10> t11;
T<(long double)2.5> t12;
T<D{{1}, 2}> t13;
T<Arr{{1, 2, 3}}> t14;
T<U{.f = 2.0f}> t15;
T<Two{1, {3}}> t16;
T<Empty{}> t17;
T<e1> t18;
T<EC::a> t19;
T<'a'> t20;
T<u8'a'> t21;
T<L'a'> t22;
T<(signed char)-3> t23;
T<(unsigned char)200> t24;
T<true> t25;
T<nullptr> t26;
T<&gi> t27;
T<fn> t28;
T<&M::x> t29;
T<&M::f> t30;
T<&ga[2]> t31;
T<(short)-5> t32;
T<5ul> t33;
T<(__int128)5> t34;
T<'\n'> t35;
P<> p0;
P<1, 'c', 2u> p1;
W<T<In{2}>> w1;
T<lam> t36;
W<decltype(lam)> w2;
T<H{{}, 3}> t37;
int use() {
  return t1.k + t2.k + t3.k + t4.k + t5.k + t6.k + t7.k + t8.k + t9.k + t10.k + t11.k +
         t12.k + t13.k + t14.k + t15.k + t16.k + t17.k + t18.k + t19.k + t20.k + t21.k +
         t22.k + t23.k + t24.k + t25.k + t26.k + t27.k + t28.k + t29.k + t30.k + t31.k +
         t32.k + t33.k + t34.k + t35.k + p0.k + p1.k + w1.k + t36.k + w2.k + t37.k;
}
