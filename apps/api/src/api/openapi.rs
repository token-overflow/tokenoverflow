use utoipa::OpenApi;

/// The schema bundle is consumed only by the build-time codegen.
/// The API does not expose the spec over HTTP.
#[derive(OpenApi)]
#[openapi(
    paths(crate::api::routes::waitlist::add_to_waitlist),
    components(schemas(crate::api::routes::waitlist::AddToWaitlistResponse)),
    info(
        title = "TokenOverflow API",
        version = "0.0.1",
        description = "TokenOverflow REST API",
    )
)]
pub struct ApiDoc;
