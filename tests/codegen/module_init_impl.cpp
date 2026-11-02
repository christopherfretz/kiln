// Itanium ABI §3.2.6, [module.unit]/8: a module implementation unit
// implicitly imports its primary interface, whose initializer it calls
// before its own initializers, from _GLOBAL__sub_I (it defines no _ZGIW).
//@ module: mod_init_a=Inputs/mod_init_a.cppm
//@ module: mod_init_b=Inputs/mod_init_b.cppm
module mod_init_b;
int vi = vb * 2;
