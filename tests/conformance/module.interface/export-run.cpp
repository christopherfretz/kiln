// [module.interface], [module.reach], [module.import]/2: an importer sees
// m's exported declarations (functions, inline functions, templates,
// classes, namespaces, constexpr variables) and can use a reachable but
// non-exported type through an exported function. m's interface is built
// in-process from its source (spec/modules.md §2) and compiled as a
// second input so its non-inline definitions are linked.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/m.cppm -fmodule-file=m=%{test_dir}/Inputs/m.cppm
//@ skip(clang): clang needs a two-step --precompile build of Inputs/m.cppm that the harness cannot drive
import m;

static_assert(sq(3) == 9);
static_assert(limit == 5);

int main() {
  if (answer() != 42)
    return 1;
  if (twice(4) != 8)
    return 2;
  Point p{1, 2};
  if (p.sum() != 3)
    return 3;
  if (ns::counter != 7)
    return 4;
  auto h = make_hidden(); // reachable, not visible: no name needed
  if (h.v != 3)
    return 5;
  if (use_gmf(1) != 2)
    return 6;
  ns::counter = 8;
  return ns::counter == 8 ? 0 : 7;
}
