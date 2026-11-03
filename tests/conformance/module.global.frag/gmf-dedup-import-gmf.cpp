// [module.global.frag]/1, [basic.def.odr]/15 (the GMF entities of two
// module units are the same entities, each unit's own declarations visible
// in it): two module interface units whose global module fragments both
// declare `std::byte` and specialize a variable template on it, the second
// re-exporting the first (libc++ 22's std.compat over std; the M5 cluster
// "std.compat GMF visibility", docs/status/libcxx-m5.md). clang builds each
// unit separately and accepts. kiln builds gdx_m in-process after gdx_c's
// GMF and merges gdx_m's GMF declarations with gdx_c's (spec/modules.md §6;
// before WP-460 it reported "redefinition of 'is_b'" and "missing
// '#include'; 'byte' must be declared before it is used" inside gdx_m).
//@ expect: compile-ok
//@ module: gdx_m=Inputs/gdx-m.cppm
//@ module: gdx_c=Inputs/gdx-c.cppm
import gdx_c;
static_assert(sizeof(std::byte) == 1);
static_assert(std::is_b<std::byte>);
int main() {}
