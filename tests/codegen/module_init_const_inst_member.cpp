// Constant-initialized static data members of a module's explicit
// instantiations (of the class, of the member) are not module initializers
// for clang 22: the importer does not call `_ZGIW` (WP-1669; a variable
// template specialization an explicit instantiation names is one,
// module_explicit_inst_importer.cpp).
//@ module: mod_init_const_inst=Inputs/mod_init_const_inst.cppm
import mod_init_const_inst;
int main() { return E<int>::y + F<int>::y; }
