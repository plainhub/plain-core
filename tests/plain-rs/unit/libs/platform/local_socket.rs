use super::*;
use std::{
    io::{Read, Write},
    os::{
        fd::AsFd,
        unix::net::{UnixListener, UnixStream},
    },
};

#[test]
fn long_application_roots_have_private_sockets_and_do_not_change_process_cwd() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("long-".repeat(30));
    fs::create_dir(&root).unwrap();
    let before = std::env::current_dir().unwrap();
    let directory = SocketDirectory::open(&root).unwrap();
    let path = directory.socket_path();
    std::os::unix::net::SocketAddr::from_pathname(&path).unwrap();
    let listener = UnixListener::bind(&path).unwrap();
    let file = directory.protect_socket().unwrap();
    assert_eq!(
        fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
        0o700
    );
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let (address, owner) = client_path(&root).unwrap();
    assert_eq!(owner, current_uid());
    let mut client = UnixStream::connect(&address).unwrap();
    let (mut accepted, _) = listener.accept().unwrap();
    assert_eq!(peer_uid(accepted.as_fd()).unwrap(), current_uid());
    assert_eq!(peer_uid(client.as_fd()).unwrap(), current_uid());
    client.write_all(b"owned").unwrap();
    let mut bytes = [0; 5];
    accepted.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"owned");
    drop(accepted);
    drop(client);
    drop(listener);
    drop(file);
    drop(directory);
    assert!(!path.parent().unwrap().exists());
    assert_eq!(std::env::current_dir().unwrap(), before);
}

#[test]
fn canonical_aliases_share_one_namespace_and_other_roots_do_not() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("data");
    let other = temporary.path().join("other");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&other).unwrap();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    assert_eq!(directory(&root).unwrap(), directory(&alias).unwrap());
    assert_ne!(directory(&root).unwrap(), directory(&other).unwrap());
}

#[test]
fn replaced_files_are_never_removed_by_socket_cleanup() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = SocketDirectory::open(temporary.path()).unwrap();
    let path = directory.socket_path();
    let listener = UnixListener::bind(&path).unwrap();
    let file = directory.protect_socket().unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"preserve replacement").unwrap();
    drop(listener);
    drop(file);
    drop(directory);
    assert_eq!(fs::read(&path).unwrap(), b"preserve replacement");
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}

#[test]
fn socket_symlinks_and_unsafe_directory_permissions_are_rejected_without_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = SocketDirectory::open(temporary.path()).unwrap();
    let path = directory.socket_path();
    let target = temporary.path().join("preserved");
    fs::write(&target, b"preserved").unwrap();
    let before = fs::metadata(&target).unwrap().permissions().mode();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(directory.protect_socket().is_err());
    assert!(client_path(temporary.path()).is_err());
    assert_eq!(fs::metadata(&target).unwrap().permissions().mode(), before);
    fs::remove_file(&path).unwrap();
    let parent = path.parent().unwrap();
    fs::set_permissions(parent, Permissions::from_mode(0o777)).unwrap();
    assert!(SocketDirectory::open(temporary.path()).is_err());
    assert_eq!(fs::metadata(parent).unwrap().mode() & 0o777, 0o777);
    fs::set_permissions(parent, Permissions::from_mode(0o700)).unwrap();
    drop(directory);
}

#[test]
fn a_changed_socket_cannot_be_removed_with_a_stale_inode() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = SocketDirectory::open(temporary.path()).unwrap();
    let path = directory.socket_path();
    let listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    let old = directory.existing_socket().unwrap().unwrap();
    directory.remove_stale(old).unwrap();
    let replacement = UnixListener::bind(&path).unwrap();
    let file = directory.protect_socket().unwrap();
    assert!(directory.remove_stale(old).is_err());
    assert!(path.exists());
    drop(listener);
    drop(replacement);
    drop(file);
    drop(directory);
}

#[test]
fn another_os_account_does_not_gain_native_approval_authority() {
    assert!(authorize_uid(501, 501).is_ok());
    assert!(authorize_uid(502, 501).is_err());
    assert!(authorize_uid(0, 501).is_err());
}
