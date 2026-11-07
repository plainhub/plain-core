macro_rules! try_vec {
    ($elem:expr; $size:expr) => {{ $crate::audio_tags::util::alloc::fallible_vec_from_element($elem, $size)? }};
}

// Shorthand for return Err(LoftyError::new(ErrorKind::Foo))
//
// Usage:
// - err!(Variant)          -> return Err(LoftyError::new(ErrorKind::Variant))
// - err!(Variant(Message)) -> return Err(LoftyError::new(ErrorKind::Variant(Message)))
macro_rules! err {
    ($variant:ident) => {
        return Err(crate::audio_tags::error::LoftyError::new(
            crate::audio_tags::error::ErrorKind::$variant,
        ))
    };
    ($variant:ident($reason:literal)) => {
        return Err(crate::audio_tags::error::LoftyError::new(
            crate::audio_tags::error::ErrorKind::$variant($reason),
        ))
    };
}

// Shorthand for FileDecodingError::new(FileType::Foo, "Message")
//
// Usage:
//
// - decode_err!(Variant, Message)
// - decode_err!(Message)
//
// or bail:
//
// - decode_err!(@BAIL Variant, Message)
// - decode_err!(@BAIL Message)
macro_rules! decode_err {
	($file_ty:ident, $reason:literal) => {
		Into::<crate::audio_tags::error::LoftyError>::into(crate::audio_tags::error::FileDecodingError::new(
			crate::audio_tags::file::FileType::$file_ty,
			$reason,
		))
	};
	($reason:literal) => {
		Into::<crate::audio_tags::error::LoftyError>::into(crate::audio_tags::error::FileDecodingError::from_description(
			$reason,
		))
	};
	(@BAIL $($file_ty:ident,)? $reason:literal) => {
		return Err(decode_err!($($file_ty,)? $reason))
	};
}

// A macro for handling the different `ParsingMode`s
//
// NOTE: All fields are optional, if `STRICT` or `RELAXED` are missing, it will
// 		 fall through to `DEFAULT`. If `DEFAULT` is missing, it will fall through
// 		 to an empty block.
//
// Usage:
//
// - parse_mode_choice!(
// 		ident_of_parsing_mode,
// 		STRICT: some_expr,
// 		RELAXED: some_expr,
// 		DEFAULT: some_expr,
// 	 )
macro_rules! parse_mode_choice {
	(
		$parse_mode:ident,
		$(STRICT: $strict_handler:expr,)?
		$(BESTATTEMPT: $best_attempt_handler:expr,)?
		$(RELAXED: $relaxed_handler:expr,)?
		DEFAULT: $default:expr
	) => {
		match $parse_mode {
			$(crate::audio_tags::config::ParsingMode::Strict => { $strict_handler },)?
			$(crate::audio_tags::config::ParsingMode::BestAttempt => { $best_attempt_handler },)?
			$(crate::audio_tags::config::ParsingMode::Relaxed => { $relaxed_handler },)?
			_ => { $default }
		}
	};
	(
		$parse_mode:ident,
		$(STRICT: $strict_handler:expr,)?
		$(BESTATTEMPT: $best_attempt_handler:expr,)?
		$(RELAXED: $relaxed_handler:expr $(,)?)?
	) => {
		match $parse_mode {
			$(crate::audio_tags::config::ParsingMode::Strict => { $strict_handler },)?
			$(crate::audio_tags::config::ParsingMode::BestAttempt => { $best_attempt_handler },)?
			$(crate::audio_tags::config::ParsingMode::Relaxed => { $relaxed_handler },)?

			_ => {}
		}
	};
}

pub(crate) use {decode_err, err, parse_mode_choice, try_vec};

macro_rules! impl_audio_file {
    ($file:ty, $properties:ty, $reader:path) => {
        impl crate::audio_tags::file::AudioFile for $file {
            type Properties = $properties;
            fn read_from<R: std::io::Read + std::io::Seek>(
                reader: &mut R,
                options: crate::audio_tags::config::ParseOptions,
            ) -> crate::audio_tags::error::Result<Self> {
                $reader(reader, options)
            }
            fn properties(&self) -> &Self::Properties {
                &self.properties
            }
        }
    };
}
pub(crate) use impl_audio_file;
