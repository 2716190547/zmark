fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=resources/windows/zmark.rc");
        println!("cargo:rerun-if-changed=../../packaging/windows/zmark.ico");
        embed_resource::compile("resources/windows/zmark.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
