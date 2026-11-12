// [temp.param]/3, [temp.arg.template]/1, [temp.spec.partial]: template
// template parameters with defaults, packs of them, partial
// specializations matching one, and a dependent template name
// (`meta<T>::template apply`) as an argument, and class template
// argument deduction for a template template parameter
// ([over.match.class.deduct]). With `--pch-diff`, the
// header's precompiled image carries them (WP-1342).
//@ expect: run
#include "Inputs/template_template_parms.h"

int main() {
  static_assert(ttp::is_spec<ttp::duo<int>, ttp::duo>);
  static_assert(!ttp::is_spec<ttp::box<int>, ttp::duo>);
  static_assert(ttp::valid<ttp::box, int>(0) && !ttp::valid<ttp::box, int, int>(0));
  static_assert(sizeof(ttp::first<ttp::duo<char, long>>::type) == 1);
  static_assert(ttp::many<ttp::box, ttp::box>::n == 2);
  ttp::wrap<int> w{{3}};
  ttp::wrap<int, ttp::box> w2{{4}};
  ttp::lazy<ttp::duo, char, short> l{};
  ttp::meta_of<char> m{};
  if (w.w.v + w2.w.v != 7 || sizeof(l) != 4 || ttp::wsize<ttp::box, char>() != 1)
    return 1;
  if (sizeof(m.q.d) != sizeof(ttp::duo<char, long>))
    return 2;
  ttp::deduce<ttp::cell, long> d{3};
  static_assert(sizeof(d) == sizeof(long));
  return d.v + ttp::make<ttp::cell>(4).v == 7 ? 0 : 3;
}
