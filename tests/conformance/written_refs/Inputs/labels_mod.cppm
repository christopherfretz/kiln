// Module unit carrying label uses (`goto`, `&&l`) in its written-reference
// table (WP-3688).
export module labels_mod;
export inline void jump_fn() {
  goto target;
target: ;
  void *p = &&target;
  (void)p;
}
export template <class T> inline void jump_tmpl(T x) {
  done: ;
  goto done;
  (void)x;
}
