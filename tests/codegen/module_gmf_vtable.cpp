// [module.global.frag]/1, [basic.def.odr]/15, Itanium ABI §5.2.3: an
// importer of a module whose global module fragment defines G's key
// function declares G's vtable, type_info and name `external`: the
// interface unit emits them, as for any class whose key function another
// translation unit defines. GI has no key function, so the importer
// emits its vtable linkonce_odr. The purview class M's vtable belongs to
// the interface unit. clang 22 and 19 instead omit G's vtable from the
// interface unit and define it strongly in every importer, so two
// importers fail to link (spec/modules.md §10 deviation 7; the DEVIATIONS
// row in ir_vs_clang.rs).
//@ min-oracle: 22
//@ module: mod_gmf_vtable=Inputs/mod_gmf_vtable.cppm
import mod_gmf_vtable;
int use() {
  G g;
  GI gi;
  M m;
  return g.f() + gi.f() + m.f();
}
