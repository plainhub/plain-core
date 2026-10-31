use super::*;
#[test]
fn make_file_id_roundtrips_through_fs_decrypt() {
    let key = [7; 32];
    let path = "app://Pictures/foo.png";
    let id = make_file_id(path, &crate::libs::utils::base64::base64_encode(&key));
    let (scope, payload) = crate::features::files::resource_link::parse(&id).unwrap();
    let resource = crate::features::files::resource_link::open(&scope, &key, payload).unwrap();
    assert_eq!(scope, crate::features::files::resource_link::Scope::Global);
    assert_eq!(
        resource,
        crate::features::files::resource_link::Resource::reference(path)
    );
}
