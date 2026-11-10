// [namespace.udecl]/1, [enum.udecl]/1, [namespace.alias]/1,
// [namespace.udir]/1, [basic.lookup.argdep]/4, [module.interface]/2,
// [module.reach]/5, spec/modules.md §11.11 (stage S6): lookup through
// exported using-declarations, a using-enum-declaration, namespace
// aliases and a using-directive of a module; ADL through a class named
// by a using-declaration finds a function of its own namespace; a
// non-exported class is reachable through an exported function's type
// without being visible. Under `--bmi-image` the importer builds the
// shadows, aliases and directives from the declaration image, and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgu=Inputs/img-using.cppm
//@ stdout: 3 11 2 2 11 | 5 16 42 9
import imgu;
extern "C" int printf(const char *, ...);
static_assert(api::twice(2) == 4 && api::twice(2L) == 6 && ali2::twice(3) == 6);
static_assert(api::get(api::Box{7}) == 7 && total() == 2 + 3 + 4 + 2);
static_assert((int)api::blue == 4 && (int)ali::red == 1 && api::green == impl::Color::green);
static_assert(api::viaalias() == 5 && api::viadir() == 16 && api::peek2() == 42);
static_assert((api::Op{1} + api::Op{2}).v == 3 && api::make(3).v == 3);
namespace mine = ali::d;
static_assert(mine::k == 5);

int main(int argc, char **) {
  api::Box b{argc + 2};
  api::Tag t;
  auto r = api::make(argc + 8);
  printf("%d %d %d %d %d | %d %d %d %d\n", api::get(b), probe(t), api::twice(argc),
         (api::Op{argc} + api::Op{1}).v, total(), api::viaalias(), api::viadir(), api::peek2(),
         r.v);
}
