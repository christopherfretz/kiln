// [dcl.attr.grammar]/6, spec/modules.md §11.11 "Attributes" (WP-1278): an
// importer of `imga` calls functions whose names mangle with an abi_tag
// (their own, and their return type's) and a hidden function, all defined
// in the module's object, and instantiates a class template member
// excluded from an explicit instantiation declaration. It links and runs
// alike when it builds `imga` from its declaration image (`--bmi-image`).
//@ expect: run
//@ std: c++20
//@ module: imga=Inputs/img-attrs.cppm
//@ stdout: 30 5 3 16 1 2 3
import imga;
extern "C" int printf(const char *, ...);
extern template struct ia::B<int>;
template struct ia::B<long>;

int main() {
  ia::B<int> b;
  printf("%d %d %d %d %d %d %d\n", ia::tagged(3), ia::make_tagged(4).v, ia::hidden_fn(4),
         ia::slow(4), b.ex(), ia::B<long>{}.in(), ia::make_res().v);
}
