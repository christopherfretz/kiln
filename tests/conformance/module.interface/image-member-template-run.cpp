// [temp.mem]/1, [temp.mem]/3, [temp.param]/12, [temp.class.general]/3,
// [module.interface], spec/modules.md §11.11 "Member templates"
// (WP-1292): member function and class templates of classes and of class
// templates exported from a module, defined in the class and out of line
// (with nested template-heads, depth 2), and templates declared before
// their definition whose definition inherits the default template
// arguments, are instantiated by the importer;
// under `--bmi-image` they come from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgmt=Inputs/img-membertempl.cppm
//@ stdout: 1 8 7 2 17 7 26 9 5 12 6 1
import imgmt;
extern "C" int printf(const char *, ...);
static_assert(mt::A::k<4>() == 8);
static_assert(mt::D<>::size == sizeof(long) && mt::D<char>::size == 1);
int main() {
  mt::A a;
  mt::A::In<short> in{2};
  mt::S<int> s{7};
  mt::S<int>::In<char> si{10, 7};
  mt::S<long>::Out<int> so{4};
  printf("%d %d %d %d %d %d %d %d %d %d %d %d\n", a.f('c'), a.f(1.0), mt::A::g<int>(), in.get(),
         si.sum(), (int)s.mul(1L), (int)si.tri(9), (int)s.add(2), (int)so.get(), mt::scaled(4),
         (int)mt::scaled<long, 2>(3), (int)(mt::D<>::size == sizeof(long)));
  return 0;
}
