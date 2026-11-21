// [dcl.attr.grammar]/6, spec/modules.md §11.11 "Attributes" (WP-1806):
// an importer of `imgam` uses declarations with GNU and clang attributes
// whose effects reach the importer's code: a `cleanup` variable in an
// inline body, a `pass_object_size` parameter, an `ms_struct` layout,
// `overloadable` C functions (mangled), and a `weak_import` variable no
// unit defines (image-gnu-alias-run.cpp uses its alias and ifunc). It
// links and runs alike when it builds `imgam` from its declaration image
// (`--bmi-image`).
//@ expect: run
//@ std: c++20
//@ module: imgam=Inputs/img-attrs-more.cppm
//@ stdout: 5 4 10 12 101 202 0 12 2
import imgam;
extern "C" int printf(const char *, ...);

int main() {
  char buf[10];
  int c = iam::clean(4);
  int cl = iam::cleaned;
  printf("%d %d %d %d %d %d %d %d %d\n", c, cl, (int)iam::osz(buf), (int)sizeof(iam::MS),
         iam_ov(1), iam_ov(2L), &iam::weak_one != nullptr, iam::ann(3), iam::in_sec(1));
}
