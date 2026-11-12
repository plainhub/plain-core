use crate::axum::error_handling::HandleError;
use crate::axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::axum::routing::IntoMakeService;
use crate::tower::Service;
pub trait ServiceExt<R>: Service<R> + Sized {
    fn into_make_service(self) -> IntoMakeService<Self>;
    fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C>;
    fn handle_error<F, T>(self, f: F) -> HandleError<Self, F, T> {
        HandleError::new(self, f)
    }
}
impl<S, R> ServiceExt<R> for S
where
    S: Service<R> + Sized,
{
    fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self)
    }
    fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self)
    }
}
