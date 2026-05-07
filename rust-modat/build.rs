fn main() {
    #[cfg(windows)]
    embed_resource::compile("src/icon.rc", embed_resource::NONE);
}
