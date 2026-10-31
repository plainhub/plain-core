#[cfg(not(any(target_os = "android", target_os = "ios")))]
use plain_rs::chat::enums::DeviceType;
#[cfg(not(target_os = "android"))]
use plain_rs::host::startup::StartupConfig;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn config() -> StartupConfig {
    #[cfg(target_os = "linux")]
    let (manufacturer, model, os_name, os_version) = {
        let model = plain_rs::platform::linux::device_info::dmi_model();
        let model = if model.is_empty() {
            std::env::var("PLAINAPP_MODEL").unwrap_or_default()
        } else {
            model
        };
        let os = plain_rs::platform::linux::device_info::os_release();
        (
            plain_rs::platform::linux::device_info::dmi_manufacturer(),
            model,
            os.clone(),
            os,
        )
    };
    #[cfg(not(target_os = "linux"))]
    let (manufacturer, model, os_name, os_version) = (
        String::new(),
        String::new(),
        sysinfo::System::name().unwrap_or_default(),
        sysinfo::System::os_version().unwrap_or_default(),
    );
    StartupConfig {
        debug: cfg!(debug_assertions),
        name: plain_rs::prefs::identity::default_device_name(),
        device_type: DeviceType::Computer,
        build_channel: "GITHUB".into(),
        downloads_dir: plain_rs::platform::download_directory::resolve()
            .and_then(|path| path.into_os_string().into_string().ok())
            .unwrap_or_default(),
        manufacturer,
        model,
        os_name,
        os_version,
        app_version: plain_rs::platform::version::version().into(),
        app_build_number: plain_rs::platform::version::full_version(),
        cpu_arch: std::env::consts::ARCH.into(),
        supported_abis: vec![std::env::consts::ARCH.into()],
        update_source: Some(plain_rs::updates::Source {
            url: "https://api.github.com/repos/plainhub/plain-desktop/releases/latest".into(),
            page_url: "https://github.com/plainhub/plain-desktop/releases/latest".into(),
            assets: Vec::new(),
        }),
        ..Default::default()
    }
}

#[cfg(target_os = "ios")]
pub fn read_device_info(config: &mut StartupConfig) -> Result<(), String> {
    let mut name = std::mem::MaybeUninit::<libc::utsname>::uninit();
    if unsafe { libc::uname(name.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let name = unsafe { name.assume_init() };
    let machine = unsafe { std::ffi::CStr::from_ptr(name.machine.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    config.manufacturer = "Apple".into();
    config.model = machine.clone();
    config.cpu_arch = machine;
    Ok(())
}
