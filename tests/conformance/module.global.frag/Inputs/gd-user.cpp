// The second importer of gd (gmf-dup-run.cpp): it uses every entity the
// first importer uses, so any definition both emitted would collide at
// link time.
import gd;
int second_user() {
  G g;
  ML l;
  MK k;
  G *p = new G;
  int r = p->f();
  delete p;
  return g.f() + l.f() + l.g() + k.f() + use_gi() + gmf_next() + r;
}
