use std::path::Path;

pub fn absolute_directory(root: &str, relative: &str) -> String {
    if relative.is_empty() {
        root.to_owned()
    } else {
        format!("{}/{relative}", root.trim_end_matches('/'))
    }
}

pub fn relative_directories(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut directories = Vec::new();
    for path in paths {
        if path.starts_with('/')
            || path.contains(['\\', '\0'])
            || Path::new(&path)
                .components()
                .any(|part| matches!(part, std::path::Component::Prefix(_)))
            || path
                .split('/')
                .any(|part| matches!(part, "." | "..") || (part.is_empty() && !path.is_empty()))
        {
            return Err("Invalid volume-relative directory".into());
        }
        directories.push(path);
    }
    directories.sort();
    directories.dedup();
    let mut result: Vec<String> = Vec::new();
    for path in directories {
        if !result
            .iter()
            .any(|root| root.is_empty() || under(&path, root))
        {
            result.push(path);
        }
    }
    Ok(result)
}

pub fn validate_directories(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for path in paths {
        let path = Path::new(&path);
        if !path.is_absolute() {
            return Err("Absolute index directory required".into());
        }
        if path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
        {
            return Err("Index directories must not contain parent components".into());
        }
        match std::fs::metadata(path) {
            Ok(metadata) if !metadata.is_dir() => return Err("Index directory required".into()),
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) => {}
            Err(error) => return Err(error.to_string()),
        }
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let path = match path.canonicalize() {
            Ok(path) => path,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                path
            }
            Err(error) => return Err(error.to_string()),
        };
        result.push(normalized(&path));
    }
    result.sort();
    result.dedup();
    Ok(result)
}

pub fn normalized(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if let Some(unc) = path.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    };
    if path == "/" || (path.len() == 3 && path.as_bytes()[1] == b':' && path.ends_with('/')) {
        path
    } else {
        path.trim_end_matches('/').to_owned()
    }
}

pub fn under(path: &str, root: &str) -> bool {
    if root == "/" {
        return path.starts_with('/');
    }
    component_eq(path, root)
        || path
            .get(..root.len())
            .is_some_and(|head| component_eq(head, root))
            && (root.ends_with('/') || path.as_bytes().get(root.len()) == Some(&b'/'))
}

pub(super) fn component_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}
