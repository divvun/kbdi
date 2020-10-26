extern crate embed_resource;

fn main() {
    // Compile and link checksums.rc
    embed_resource::compile("kbdi.rc");
}
