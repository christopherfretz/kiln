// Interface unit of module `cav` for carried-alias-value-arg-run.cpp
// (spec/modules.md §11.11, WP-1168): constexpr reflections of alias
// template specializations with non-type arguments written as
// expressions, evaluated here, so a kind-2 module file carries them by
// their converted arguments.
module;
#include "kiln_meta.h"
export module cav;
export using std::meta::info;
export template <class T, int N> using Arr = T[N];
export enum class E { a, b };
export template <E e, bool B> using EB = int[int(e) + 2 * B + 1];
export constexpr info rarr = ^^Arr<char, 1 + 2>;
export constexpr info rarr_s = ^^Arr<char, (short)3>;
export constexpr info reb = ^^EB<static_cast<E>(1), 1>;
