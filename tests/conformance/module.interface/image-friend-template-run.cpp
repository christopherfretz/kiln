// [temp.friend]/1, [class.friend]/2, [namespace.memdef]/3,
// [basic.lookup.argdep]/4, [module.interface], spec/modules.md §11.11
// "Friends" (WP-1307): friend templates of a class and of a class
// template, a friend template that first declares a class template
// defined later, hidden friends of a class template found by
// argument-dependent lookup, and befriended classes reading private
// members, are used by the importer; under `--bmi-image` they come from
// the declaration image and the program behaves exactly as with the
// replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgft=Inputs/img-friend-template.cppm
//@ stdout: 6 18 105 9 | 1 97 0 0 5 9 15
import imgft;
extern "C" int printf(const char *, ...);
static_assert(f(1, ft::A(2)) == 2 + (int)sizeof(int));
int main() {
  ft::A a(5);
  printf("%d %d %d %d | ", f('c', a), g(1.0, a), ft::B<int>::get(a), f(0, ft::A(5)));
  char buf[4] = {'a', 'b', 'c', 0};
  ft::Str<char> s(buf);
  ft::It<char> i = s.begin(), j = s.begin();
  int arr[2] = {7, 8};
  ft::Str<int, long> t(arr);
  printf("%d %d %d %d %d %d %d\n", i == j, peek(i), dist(i, j),
         (int)(ft::Probe::get(t.begin()) - arr), s.size(), t.size(), ft::Early<int>::get());
  return 0;
}
