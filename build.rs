extern crate capnpc;

fn main() {
    ::capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/msg.capnp")
        .run().expect("schema compiler command");
}
