#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AadPurpose {
    AuthRequest,
    AuthResponse,
    HttpRequest,
    WsInit,
    WsAck,
    WsFrame,
}

impl AadPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequest => "AUTH_REQUEST",
            Self::AuthResponse => "AUTH_RESPONSE",
            Self::HttpRequest => "HTTP_REQUEST",
            Self::WsInit => "WS_INIT",
            Self::WsAck => "WS_ACK",
            Self::WsFrame => "WS_FRAME",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourcePurpose {
    Fs,
    ZipFiles,
}

impl ResourcePurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fs => "FS",
            Self::ZipFiles => "ZIP_FILES",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginPurpose {
    Password,
    NoPassword,
}

impl LoginPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "LOGIN_PASSWORD",
            Self::NoPassword => "LOGIN_NO_PASSWORD",
        }
    }
}
