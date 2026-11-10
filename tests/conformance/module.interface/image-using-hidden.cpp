// [module.interface]/2, [namespace.udecl]/1, [module.reach]/5,
// spec/modules.md §11.11 (stage S6): a non-exported using-declaration
// of a module is not visible to an importer (its shadow has the
// using-declaration's owner), and neither is a non-exported function or
// class, reachable or not; the errors are the same whether the importer
// builds the module from its declaration image or replays it.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgu=Inputs/img-using.cppm
//@ diag: 14:14 error "declaration of 'secret' must be imported from module 'imgu' before it is required"
//@ diag: 15:14 error "declaration of 'peek' must be imported from module 'imgu' before it is required"
//@ diag: 16:7 error "declaration of 'Raw' must be imported from module 'imgu' before it is required"
import imgu;
int a = api::secret();
int b = api::peek();
priv::Raw r = api::make(1);
