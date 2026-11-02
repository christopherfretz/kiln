// Module `b` re-exports `a` ([module.import]/7).
export module b;
export import a;
export int fb() { return fa() + 10; }
