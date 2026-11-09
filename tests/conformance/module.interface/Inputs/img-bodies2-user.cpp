// The second importer of module `imgc` (image-bodies-odr-run.cpp): its
// calls to the inline `imgc::next` share the one static local.
import imgc;
int user_next() { return imgc::next() * 10; }
const char *user_greeting() { return imgc::greeting(); }
