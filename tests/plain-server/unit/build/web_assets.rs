use super::*;

#[test]
fn generator_sorts_keys_tracks_nested_assets_and_excludes_hidden_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::create_dir(root.join(".private")).unwrap();
    fs::write(root.join("index.html"), b"<html><head></head></html>").unwrap();
    fs::write(root.join("assets/z.js"), [0, 255, 42]).unwrap();
    fs::write(root.join("assets/a.css"), b"body{}").unwrap();
    fs::write(root.join(".secret"), b"secret").unwrap();
    fs::write(root.join(".private/token"), b"secret").unwrap();
    let output = dir.path().join("generated.rs");
    generate(&root, &output, true).unwrap();
    let generated = fs::read_to_string(&output).unwrap();
    assert!(generated.find("assets/a.css").unwrap() < generated.find("assets/z.js").unwrap());
    assert!(generated.find("assets/z.js").unwrap() < generated.find("index.html").unwrap());
    assert_eq!(generated.matches("include_bytes!").count(), 3);
    assert!(!generated.contains("secret"));
    assert!(!generated.contains("token"));
    fs::remove_file(root.join("assets/z.js")).unwrap();
    fs::write(root.join("assets/new.js"), b"new").unwrap();
    generate(&root, &output, true).unwrap();
    let generated = fs::read_to_string(output).unwrap();
    assert!(!generated.contains("assets/z.js"));
    assert!(generated.contains("assets/new.js"));
}

#[test]
fn explicit_missing_or_incomplete_bundles_fail_instead_of_silently_omitting_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    let output = dir.path().join("generated.rs");
    assert!(generate(&root, &output, true).is_err());
    assert!(!output.exists());
    generate(&root, &output, false).unwrap();
    assert!(
        !fs::read_to_string(&output)
            .unwrap()
            .contains("include_bytes!")
    );
    fs::create_dir(&root).unwrap();
    fs::write(root.join("only.js"), b"incomplete").unwrap();
    for required in [false, true] {
        assert!(generate(&root, &output, required).is_err());
    }
}

#[cfg(unix)]
#[test]
fn generator_rejects_symbolic_links_to_external_files_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("index.html"), b"shell").unwrap();
    let outside = dir.path().join("secret");
    fs::write(&outside, b"secret").unwrap();
    let output = dir.path().join("generated.rs");
    let link = root.join("linked");
    for target in [&outside, dir.path()] {
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert!(generate(&root, &output, true).is_err());
        assert!(!output.exists());
        fs::remove_file(&link).unwrap();
    }
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    assert!(generate(&alias, &output, true).is_err());
}

#[test]
fn canonical_icons_override_generated_copies_and_survive_missing_optional_web_output() {
    let dir = tempfile::tempdir().unwrap();
    let web = dir.path().join("web_dist");
    let icons = dir.path().join("assets/ficons");
    fs::create_dir_all(web.join("ficons")).unwrap();
    fs::create_dir_all(&icons).unwrap();
    fs::write(web.join("index.html"), b"shell").unwrap();
    fs::write(web.join("ficons/default.svg"), b"stale").unwrap();
    for name in ["default.svg", "folder.svg"] {
        fs::write(icons.join(name), format!("<svg>{name}</svg>")).unwrap();
    }
    let output = dir.path().join("generated.rs");
    generate_with_icons(&web, &icons, &output, true).unwrap();
    let generated = fs::read_to_string(&output).unwrap();
    assert_eq!(generated.matches("(\"ficons/default.svg\"").count(), 1);
    assert!(!generated.contains(web.join("ficons/default.svg").to_str().unwrap()));
    assert!(generated.contains(icons.join("default.svg").to_str().unwrap()));
    fs::remove_dir_all(&web).unwrap();
    generate_with_icons(&web, &icons, &output, false).unwrap();
    let generated = fs::read_to_string(&output).unwrap();
    assert_eq!(generated.matches("include_bytes!").count(), 2);
    assert_eq!(
        fs::read(icons.join("default.svg")).unwrap(),
        b"<svg>default.svg</svg>"
    );
    fs::remove_file(icons.join("folder.svg")).unwrap();
    assert!(generate_with_icons(&web, &icons, &output, false).is_err());
}
