// [temp.dep.candidate], [class.member.lookup], [temp.inst]/4,
// [module.interface], spec/modules.md §11.11 (WP-1314): an imported
// class template's calls of member function templates and its dependent
// decltype base resolve at each instantiation; under `--bmi-image` the
// declaration image carries the found members and their naming class.
//@ expect: run
//@ std: c++20
//@ module: imgmn=Inputs/img-member-names.cppm
import imgmn;
struct G {
  int operator()(int x) const { return x; }
};
int main() {
  int r = mf::holder<int>::go(3) + mf::holder<long>().inst(4L);
  if (r != 18 || !mf::is_ref2<int>::value || mf::is_ref2<void>::value)
    return 1;
  mf::inv<G>::R x = 5;
  return x == 5 ? 0 : 2;
}
