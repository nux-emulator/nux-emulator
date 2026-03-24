//! Generated Wayland protocol bindings for virtio-gpu-metadata.

#![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#![allow(non_upper_case_globals, non_snake_case, unused_imports)]
#![allow(missing_docs, clippy::all)]

// Follow the exact pattern from wayland-protocols' protocol_macro.rs
pub mod server {
    use wayland_server;
    use wayland_server::protocol::*;

    pub mod __interfaces {
        use wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocol/virtio-gpu-metadata-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("protocol/virtio-gpu-metadata-v1.xml");
}

pub use server::wp_virtio_gpu_metadata_v1;
pub use server::wp_virtio_gpu_surface_metadata_v1;
