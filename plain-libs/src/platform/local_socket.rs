use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, Metadata, Permissions},
    io,
    os::{
        fd::BorrowedFd,
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        },
    },
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SocketIdentity {
    device: u64,
    inode: u64,
    owner: u32,
}

impl SocketIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            owner: metadata.uid(),
        }
    }
}

pub fn directory(data_root: &Path) -> io::Result<PathBuf> {
    let root = data_root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Application directory required",
        ));
    }
    let hash = Sha256::digest(root.as_os_str().as_bytes());
    let name = format!(
        "plainapp-login-{}",
        crate::utils::hex::bytes_to_hex(&hash[..16])
    );
    Ok(Path::new("/tmp").canonicalize()?.join(name))
}

pub fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

pub fn authorize_uid(actual: u32, expected: u32) -> io::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Run this command as the service OS account",
        ))
    }
}

fn private_directory(path: &Path) -> io::Result<Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.is_symlink() || metadata.mode() & 0o7777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Unsafe local socket directory",
        ));
    }
    Ok(metadata)
}

fn socket_identity(path: &Path, owner: u32, private: bool) -> io::Result<Option<SocketIdentity>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != owner
        || private && metadata.mode() & 0o7777 != 0o600
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Unsafe local socket file",
        ));
    }
    Ok(Some(SocketIdentity::from_metadata(&metadata)))
}

pub struct SocketDirectory {
    path: PathBuf,
    identity: SocketIdentity,
}

impl SocketDirectory {
    pub fn open(data_root: &Path) -> io::Result<Self> {
        let path = directory(data_root)?;
        match DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = private_directory(&path)?;
        authorize_uid(metadata.uid(), current_uid())?;
        Ok(Self {
            path,
            identity: SocketIdentity::from_metadata(&metadata),
        })
    }

    pub fn socket_path(&self) -> PathBuf {
        self.path.join("login.sock")
    }

    pub fn existing_socket(&self) -> io::Result<Option<SocketIdentity>> {
        socket_identity(&self.socket_path(), self.identity.owner, true)
    }

    pub fn remove_stale(&self, identity: SocketIdentity) -> io::Result<()> {
        if self.existing_socket()? != Some(identity) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Local socket changed",
            ));
        }
        fs::remove_file(self.socket_path())
    }

    pub fn protect_socket(&self) -> io::Result<SocketFile> {
        let path = self.socket_path();
        let identity = socket_identity(&path, self.identity.owner, false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Local socket is missing"))?;
        let file = SocketFile { path, identity };
        fs::set_permissions(&file.path, Permissions::from_mode(0o600))?;
        Ok(file)
    }
}

impl Drop for SocketDirectory {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
            metadata.is_dir() && SocketIdentity::from_metadata(&metadata) == self.identity
        }) {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

pub struct SocketFile {
    path: PathBuf,
    identity: SocketIdentity,
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        if socket_identity(&self.path, self.identity.owner, false)
            .is_ok_and(|identity| identity == Some(self.identity))
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn client_path(data_root: &Path) -> io::Result<(PathBuf, u32)> {
    let directory = directory(data_root)?;
    let owner = private_directory(&directory)?.uid();
    authorize_uid(current_uid(), owner)?;
    let path = directory.join("login.sock");
    socket_identity(&path, owner, true)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Local approval service is not running",
        )
    })?;
    Ok((path, owner))
}

pub fn peer_uid(fd: BorrowedFd<'_>) -> io::Result<u32> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "linux")]
    unsafe {
        let mut credentials: libc::ucred = std::mem::zeroed();
        let mut size = std::mem::size_of_val(&credentials) as libc::socklen_t;
        if libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut size,
        ) != 0
        {
            return Err(io::Error::last_os_error());
        }
        if size as usize != std::mem::size_of_val(&credentials) {
            return Err(io::Error::other("Invalid local peer credentials"));
        }
        Ok(credentials.uid)
    }
    #[cfg(target_os = "macos")]
    unsafe {
        let mut uid = 0;
        let mut gid = 0;
        if libc::getpeereid(fd.as_raw_fd(), &mut uid, &mut gid) != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/platform/local_socket.rs"]
mod tests;
