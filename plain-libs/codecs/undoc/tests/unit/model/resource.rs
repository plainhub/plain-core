use super::*;

#[test]
fn test_resource_type_from_mime() {
    assert_eq!(
        ResourceType::from_mime_type("image/png"),
        ResourceType::Image
    );
    assert_eq!(
        ResourceType::from_mime_type("IMAGE/JPEG"),
        ResourceType::Image
    );
    assert_eq!(
        ResourceType::from_mime_type("audio/mpeg"),
        ResourceType::Audio
    );
    assert_eq!(
        ResourceType::from_mime_type("video/mp4"),
        ResourceType::Video
    );
    assert_eq!(
        ResourceType::from_mime_type("application/octet-stream"),
        ResourceType::Other
    );
}

#[test]
fn test_resource_type_from_extension() {
    assert_eq!(ResourceType::from_extension("png"), ResourceType::Image);
    assert_eq!(ResourceType::from_extension("JPG"), ResourceType::Image);
    assert_eq!(ResourceType::from_extension("mp3"), ResourceType::Audio);
    assert_eq!(ResourceType::from_extension("mp4"), ResourceType::Video);
    assert_eq!(ResourceType::from_extension("xyz"), ResourceType::Other);
}

#[test]
fn test_image_dimensions_from_headers() {
    let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
    png.extend_from_slice(&640u32.to_be_bytes());
    png.extend_from_slice(&480u32.to_be_bytes());
    assert_eq!(image_dimensions(&png), Some((640, 480)));

    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[0x20, 0x01, 0x10, 0x00]);
    assert_eq!(image_dimensions(&gif), Some((288, 16)));

    let mut bmp = vec![0u8; 26];
    bmp[..2].copy_from_slice(b"BM");
    bmp[18..22].copy_from_slice(&100i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&(-50i32).to_le_bytes()); // top-down
    assert_eq!(image_dimensions(&bmp), Some((100, 50)));

    // SOI, an APP0 segment, then SOF0 with height 300 and width 400.
    let jpeg = [
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01, 0x2C,
        0x01, 0x90,
    ];
    assert_eq!(image_dimensions(&jpeg), Some((400, 300)));

    assert_eq!(image_dimensions(b"<svg/>"), None);
    assert_eq!(
        image_dimensions(b"\x89PNG\r\n\x1a\n"),
        None,
        "truncated header"
    );
    assert_eq!(
        image_dimensions(&[0xFF, 0xD8, 0xFF]),
        None,
        "truncated JPEG"
    );
}

#[test]
fn test_from_part_types_by_extension_and_reads_size() {
    let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&3u32.to_be_bytes());
    let r = Resource::from_part("word/media/image1.PNG", png);
    assert_eq!(r.filename.as_deref(), Some("image1.PNG"));
    assert_eq!(r.resource_type, ResourceType::Image);
    assert_eq!(r.mime_type.as_deref(), Some("image/png"));
    assert_eq!((r.width, r.height), (Some(2), Some(3)));
    assert_eq!(r.role, ResourceRole::Primary);

    let wdp = Resource::from_part("ppt/media/hdphoto1.wdp", vec![1, 2]);
    assert_eq!(wdp.resource_type, ResourceType::Image);
    assert_eq!(wdp.mime_type.as_deref(), Some("image/vnd.ms-photo"));
    assert_eq!((wdp.width, wdp.height), (None, None));
}

#[test]
fn test_resource_creation() {
    let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic
    let resource = Resource::image(data.clone(), Some("test.png".to_string()));

    assert_eq!(resource.resource_type, ResourceType::Image);
    assert_eq!(resource.size, 4);
    assert_eq!(resource.filename, Some("test.png".to_string()));
    assert_eq!(resource.mime_type, Some("image/png".to_string()));
}

#[test]
fn test_resource_extension() {
    let resource = Resource::image(vec![], Some("image.png".to_string()));
    assert_eq!(resource.extension(), Some("png"));

    let resource2 = Resource::image(vec![], Some("photo.JPEG".to_string()));
    assert_eq!(resource2.extension(), Some("JPEG"));
}

#[test]
fn test_suggested_filename() {
    let resource = Resource::image(vec![], Some("original.png".to_string()));
    assert_eq!(resource.suggested_filename("img1"), "original.png");

    let mut resource2 = Resource::new(ResourceType::Image, vec![]);
    resource2.mime_type = Some("image/jpeg".to_string());
    assert_eq!(resource2.suggested_filename("img2"), "img2.jpg");
}

#[test]
fn test_is_image() {
    let image = Resource::new(ResourceType::Image, vec![]);
    assert!(image.is_image());

    let chart = Resource::new(ResourceType::Chart, vec![]);
    assert!(chart.is_image());

    let audio = Resource::new(ResourceType::Audio, vec![]);
    assert!(!audio.is_image());
}

#[test]
fn test_is_media() {
    let audio = Resource::new(ResourceType::Audio, vec![]);
    assert!(audio.is_media());

    let video = Resource::new(ResourceType::Video, vec![]);
    assert!(video.is_media());

    let image = Resource::new(ResourceType::Image, vec![]);
    assert!(!image.is_media());
}
