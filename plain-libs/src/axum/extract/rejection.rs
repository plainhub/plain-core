pub use crate::axum::extract::path::{FailedToDeserializePathParams, InvalidUtf8InPathParam};
use crate::axum_core::__composite_rejection as composite_rejection;
use crate::axum_core::__define_rejection as define_rejection;
pub use crate::axum_core::extract::rejection::*;
define_rejection! { #[status = UNPROCESSABLE_ENTITY] #[body = "Failed to deserialize the JSON body into the target type"] #[cfg_attr (docsrs , doc (cfg (all ())))] pub struct JsonDataError (Error) ; }
define_rejection! { #[status = BAD_REQUEST] #[body = "Failed to parse the request body as JSON"] #[cfg_attr (docsrs , doc (cfg (all ())))] pub struct JsonSyntaxError (Error) ; }
define_rejection! { #[status = UNSUPPORTED_MEDIA_TYPE] #[body = "Expected request with `Content-Type: application/json`"] #[cfg_attr (docsrs , doc (cfg (all ())))] pub struct MissingJsonContentType ; }
define_rejection! { #[status = INTERNAL_SERVER_ERROR] #[body = "Missing request extension"] pub struct MissingExtension (Error) ; }
define_rejection! { #[status = INTERNAL_SERVER_ERROR] #[body = "No paths parameters found for matched route"] pub struct MissingPathParams ; }
define_rejection! { #[status = UNSUPPORTED_MEDIA_TYPE] #[body = "Form requests must have `Content-Type: application/x-www-form-urlencoded`"] pub struct InvalidFormContentType ; }
define_rejection! { #[status = BAD_REQUEST] #[body = "No host found in request"] pub struct FailedToResolveHost ; }
define_rejection! { #[status = BAD_REQUEST] #[body = "Failed to deserialize form"] pub struct FailedToDeserializeForm (Error) ; }
define_rejection! { #[status = UNPROCESSABLE_ENTITY] #[body = "Failed to deserialize form body"] pub struct FailedToDeserializeFormBody (Error) ; }
define_rejection! { #[status = BAD_REQUEST] #[body = "Failed to deserialize query string"] pub struct FailedToDeserializeQueryString (Error) ; }
composite_rejection! { pub enum QueryRejection { FailedToDeserializeQueryString , } }
composite_rejection! { pub enum FormRejection { InvalidFormContentType , FailedToDeserializeForm , FailedToDeserializeFormBody , BytesRejection , } }
composite_rejection! { pub enum RawFormRejection { InvalidFormContentType , BytesRejection , } }
composite_rejection! { #[cfg_attr (docsrs , doc (cfg (all ())))] pub enum JsonRejection { JsonDataError , JsonSyntaxError , MissingJsonContentType , BytesRejection , } }
composite_rejection! { pub enum ExtensionRejection { MissingExtension , } }
composite_rejection! { pub enum PathRejection { FailedToDeserializePathParams , MissingPathParams , } }
composite_rejection! { pub enum RawPathParamsRejection { InvalidUtf8InPathParam , MissingPathParams , } }
composite_rejection! { pub enum HostRejection { FailedToResolveHost , } }
define_rejection! { #[status = INTERNAL_SERVER_ERROR] #[body = "The matched route is not nested"] pub struct NestedPathRejection ; }
