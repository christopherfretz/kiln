// [temp.expl.spec], [temp.spec.partial], [temp.inst], [temp.explicit],
// [module.interface], spec/modules.md §11.11 (WP-1352): an imported
// module's explicit and partial specializations declared before their
// definitions, variable template specializations, a class template as a
// template argument in a body, explicit specializations and explicit
// instantiation declarations of function templates and of a member;
// under `--bmi-image` they come from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++26
//@ module: imgds=Inputs/img-declared-specializations.cppm
import imgds;

template int ds::S<int>::get() const;

int main() {
  if (ds::ch<char>::size + ds::other{}.k != 2 || ds::pr<int, int>::v + ds::pr<int, char>::v != 2)
    return 1;
  if (ds::e != 2.5 || ds::e_v<float> != 2.5f || ds::use() != int(sizeof(int)))
    return 2;
  return ds::pick() + ds::inc(5) + ds::dbl(3) == 0 + 4 + 1 + 4 + 6 ? 0 : 3;
}
