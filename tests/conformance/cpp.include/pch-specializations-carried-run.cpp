// [temp.arg.general]/9, [temp.spec.partial]/9, [temp.class.general],
// [class.static.data]/4: specializations whose arguments hold a pack,
// partial specializations matching a non-type parameter pack's
// expansion, class templates deriving from their own specializations,
// dependent specializations of the builtin templates, and a default
// template argument that reads a static data member while a member of
// a completed specialization names it. With `--pch-diff`, the header's
// snapshot carries them (WP-1344).
//@ expect: run
#include "Inputs/specializations_carried.h"

int main() {
  r1::pick<int>::type a = 3;
  r6::impl<int, char>::type b = 2;
  r7::add<r7::seq<1, 2>, 3>::type s;
  (void)s;
  r9::mk<3>::type t;
  (void)t;
  r9::nth<1, char, int>::type x = 2;
  if (sizeof(a) != 4 || b + r6::chain<int>::depth != 5)
    return 1;
  if (r7::au<2, char, int, short>::len != 4 || r9::make_seq<int, 4>::n != 4 || x != 2)
    return 2;
  return r4::C<int>::t::v + r4::C<char>::t::v == 3 ? 0 : 3;
}
