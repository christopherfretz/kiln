// [module.import]/7, [module.interface]/2, [module.reach]/5,
// spec/modules.md §11.11 (stage S6): imgr re-exports imgp, whose primary
// re-exports :a but only imports :b, and imgr imports imgu without
// re-exporting it. The consumer sees none of :b's names, :a's
// module-linkage names, or imgu's exports; top::Wrap stays reachable
// through an exported function's type. The errors are the same with the
// image on and off.
//@ expect: compile-fail
//@ std: c++20
//@ module: imgu=Inputs/img-using.cppm
//@ module: imgp:a=Inputs/img-part-a.cppm
//@ module: imgp:b=Inputs/img-part-b.cppm
//@ module: imgp=Inputs/img-part.cppm
//@ module: imgr=Inputs/img-part-reexp.cppm
//@ diag: 19:13 error "declaration of 'triple' must be imported from module 'imgp' before it is required"
//@ diag: 20:13 error "declaration of 'scale' must be imported from module 'imgp' before it is required"
//@ diag: 21:14 error "declaration of 'peek2' must be imported from module 'imgu' before it is required"
import imgr;
int a = pb::triple(1);
int b = pa::scale;
int c = api::peek2();
int d = top::unwrap({4});
