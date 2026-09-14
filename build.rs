extern crate embed_resource;

fn main() {
    println!("cargo:rerun-if-changed=kbdi.rc");
    println!("cargo:rerun-if-changed=kbdi.exe.manifest");
    // Compile and link the installer resources for the selected Cargo target.
    embed_resource::compile("kbdi.rc", embed_resource::NONE)
        .manifest_required()
        .expect("compile keyboard installer resources");
}
