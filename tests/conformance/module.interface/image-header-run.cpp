// [module.interface]/2, [cpp.include]/7, spec/modules.md §11.11
// "File table" (stage S7): a module whose purview #includes a header of
// functions, classes and function templates. Under `--bmi-image` the
// importer builds `imgh` from the image, the header's locations in the
// file table; the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgh=Inputs/img-hdr.cppm
//@ stdout: 3 -1 7 12 10 12 7 9 16
import imgh;
extern "C" int printf(const char *, ...);
static_assert(hh::add(1, 2) == 3 && hh::twice(5) == 10 && hh::times<int, 3>(4) == 12);
static_assert(hh::Point{3, 4}.sum() == 7 && hh::get_seven() == 7 && hh::get_nine() == 9);

int main() {
  hh::Point p{1, 2};
  printf("%d %d %d %d %d %d %d %d %d\n", hh::add(1, 2), hh::sub(1, 2), hh::Point{3, 4}.sum(),
         p.scaled(6), hh::twice(5), hh::times<long, 3>(4), hh::get_seven(), hh::get_nine(),
         sq(4));
}
