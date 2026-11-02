// Module `pv` with a private module fragment ([module.private.frag]).
export module pv;
export int pv_get();
module :private;
int pv_secret = 41;
int pv_get() { return pv_secret + 1; }
