// [expr.prim.lambda.closure]/3-5, [temp.mem], [temp.inst], [module.interface],
// spec/modules.md §11.11 "Forms carried by WP-1350" (WP-3410): generic
// lambdas in exported function templates, a class template's member
// (function template), nested ones, with an explicit template parameter
// list or a constrained placeholder, in a default argument and a variable
// template's initializer, converted to a pointer to function and called in
// constant evaluation. The importer instantiates the templates for several
// types and calls each closure with several argument types; under
// `--bmi-image` the interface is imported from its declaration image (no
// replay; kind2_declaration_image_generic_lambdas_in_templates checks it
// with the source blanked) and the program behaves exactly as replayed.
//@ expect: run
//@ std: c++20
//@ module: imggl=Inputs/img-generic-lambda-templ.cppm
//@ stdout: 3 4 5 97 0.5 | 12 15 6 9 | 6 5 6 7 | 6 7 3 | 5 9 8 6 | 5 3 5 5 9
import imggl;
extern "C" int printf(const char*, ...);
struct S {
  int v;
};
static_assert(gl::ce(4) == 12 && gl::ce(2L) == 6);

int main() {
  auto r = gl::ret(3);
  auto n = gl::nest(1)(2);
  int i = gl::id(3);
  int s = gl::id(S{4}).v;
  int d = int(gl::id(2.5) * 2);
  printf("%d %d %d %d %g | ", i, s, d, gl::id('a'), gl::id(0.5));
  printf("%d %d %d %d | ", gl::both(1), int(gl::both(2L)), r(2), int(r(1.5) * 2));
  int c1 = gl::C<int>{5}.m();
  int c2 = int(gl::C<double>{1.5}.m() * 2);
  int c3 = gl::C<int>{1}.mt(3);
  int c4 = int(gl::C<long>{1}.mt(0.5) * 2);
  printf("%d %d %d %d | ", c1, c2, c3, c4);
  printf("%d %d %d | ", n(3), int(n(0.5) * 2), int(gl::nest(1L)(1)(1)));
  printf("%d %d %d %d | ", gl::expl(1), int(gl::expl(1.0)), gl::cons(4), int(gl::cons(3L)));
  int v1 = gl::vt<int>(4);
  int v2 = int(gl::vt<double>(1.5) * 2);
  int cv = gl::conv(1) + gl::conv(2L);
  printf("%d %d %d %d %d\n", gl::dflt(4), gl::dflt(2.0), v1, v2, cv);
  return 0;
}
