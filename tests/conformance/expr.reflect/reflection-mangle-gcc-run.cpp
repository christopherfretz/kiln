// [temp.type]/2, [temp.arg.nontype]/2 P2996R13; spec/reflection.md §8.3
// (WP-879): under -fkiln-reflection-mangling=gcc a std::meta::info
// template argument is mangled by its reflectee in GCC 16.1's form,
// `L Dm <kind> <operand> E`, so the type names below are byte-identical to
// g++-16's (the output is compared with GCC 16.1). One class template specialization per
// reflection kind: null, type, alias, namespace, global namespace,
// namespace alias, variable, function, data member, unnamed bit-field,
// enumerator, class/function/variable/alias template, concept,
// structured binding, parameter, base, value, object, data member spec
// and annotation; then the info type itself (`Dm`, not a substitution
// candidate) and two equal reflections in one argument list.
//@ expect: run
//@ args: -freflection -fkiln-reflection-mangling=gcc --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 1KILDmnuEE
//@ stdout: 1KILDmtyiEE
//@ stdout: 1KILDmtyPKiEE
//@ stdout: 1KILDmty2TTIiEEE
//@ stdout: 1KILDmta1I_iEE
//@ stdout: 1KILDmns2nsEE
//@ stdout: 1KILDmgsEE
//@ stdout: 1KILDmna2naEE
//@ stdout: 1KILDmvr3varEE
//@ stdout: 1KILDmvrN1S2smEEE
//@ stdout: 1KILDmfn1gvEE
//@ stdout: 1KILDmfnN1S2mfEvEE
//@ stdout: 1KILDmdm1S2dmEE
//@ stdout: 1KILDmdm2ns1S1bEE
//@ stdout: 1KILDmen1S1E2e1EE
//@ stdout: 1KILDmen2EC1bEE
//@ stdout: 1KILDmct2TTEE
//@ stdout: 1KILDmft2tfEE
//@ stdout: 1KILDmvt2tvEE
//@ stdout: 1KILDmat2TAEE
//@ stdout: 1KILDmco1CEE
//@ stdout: 1KILDmsb3sb2EE
//@ stdout: 1KILDmpa0_3pfnidEE
//@ stdout: 1KILDmba0_2D2EE
//@ stdout: 1KILDmvlLi42EEE
//@ stdout: 1KILDmvlL2EC0EEE
//@ stdout: 1KILDmvlLDnEEE
//@ stdout: 1KILDmvlLDmtyiEEE
//@ stdout: 1KILDmobL_Z3varEEE
//@ stdout: 1KILDmdsi_1x___EE
//@ stdout: 1KILDman_EE
//@ stdout: Dm
//@ stdout: 2TTIDmE
//@ stdout: 3TwoILDmtyiELDmtyiEE
//@ stdout: 3TwoILDmty1SELDmdmS0_2dmEE
#include <cstdio>
#include <meta>
#include <typeinfo>
using namespace std::meta;

template <info R> struct K {};
template <info A, info B> struct Two {};
struct S { int dm; static int sm; int mf(); enum E { e1 }; };
int S::sm = 0;
enum class EC { a, b };
using I = int;
int var;
int g() { return 0; }
namespace ns { struct S { int dm; int : 3; int b : 2; }; }
namespace na = ns;
template <class T> struct TT {};
template <class T> int tf(T);
template <class T> int tv = 0;
template <class T> concept C = true;
template <class T> using TA = T*;
struct B1 {}; struct B2 {}; struct D2 : B1, B2 {};
int pfn(int p0, double p1);
struct [[=1]] Ann {};
int arr[2] = {1, 2};
auto [sb1, sb2] = arr;

#define N(...) std::printf("%s\n", typeid(__VA_ARGS__).name())

int main() {
  N(K<info{}>); N(K<^^int>); N(K<^^const int*>); N(K<^^TT<int>>); N(K<^^I>);
  N(K<^^ns>); N(K<^^::>); N(K<^^na>); N(K<^^var>); N(K<^^S::sm>);
  N(K<^^g>); N(K<^^S::mf>); N(K<^^S::dm>);
  N(K<nonstatic_data_members_of(^^ns::S, access_context::unchecked())[1]>);
  N(K<^^S::e1>); N(K<^^EC::b>); N(K<^^TT>); N(K<^^tf>); N(K<^^tv>);
  N(K<^^TA>); N(K<^^C>); N(K<^^sb2>); N(K<parameters_of(^^pfn)[1]>);
  N(K<bases_of(^^D2, access_context::unchecked())[1]>);
  N(K<reflect_constant(42)>); N(K<reflect_constant(EC::a)>);
  N(K<reflect_constant(nullptr)>); N(K<reflect_constant(^^int)>);
  N(K<reflect_object(var)>); N(K<data_member_spec(^^int, {.name = "x"})>);
  N(K<annotations_of(^^Ann)[0]>);
  N(info); N(TT<info>); N(Two<^^int, ^^int>); N(Two<^^S, ^^S::dm>);
}
