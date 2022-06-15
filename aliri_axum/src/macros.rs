//! Types used to assert that a presented token is authorized to access protected API scopes

/// Constructs an extractor that enables easily asserting that a provided token
/// has the expected set of scopes.
///
/// For an more concise way to construct several scope guards, see [`scope_guards!`].
///
/// In the simplest case, a single scope can be used:
///
/// ```
/// use aliri_axum::scope_guard;
///
/// scope_guard!(ReadProfile; "read:profile");
/// ```
///
/// In more complex scenarios, multiple sets of scopes can be accepted by separating sets with
/// the logical or operator (`||`):
///
/// ```
/// use aliri_axum::scope_guard;
///
/// scope_guard!(
///     ReadProfileOrAdmin;
///     ["read:profile" || "admin"]
/// );
/// ```
///
/// In situations where multiple scope tokens must all be present, they should be combined into
/// a single space-separated scope:
///
/// ```
/// use aliri_axum::scope_guard;
///
/// scope_guard!(
///     DeleteProfileAndAdmin;
///     "delete:profile admin"
/// );
/// ```
///
/// These two different forms can be combined to add more complex scope guards:
///
/// ```
/// use aliri_axum::scope_guard;
///
/// scope_guard!(
///     DeleteProfileAndAdminOrSuperAdmin;
///     [ "delete:profile admin" || "super_admin" ]
/// );
/// ```
///
/// These scope guards can then be used on an axum handler endpoint in order to assert that
/// the presented JWT token is valid according to the configured authority _and_ that it
/// has the necessary scopes.
///
/// These handlers will expect that the relevant claims have already been validated and placed
/// into the request's extensions.
///
/// ```no_run
/// use aliri_axum::scope_guard;
/// use axum::routing::get;
/// use axum::{Router, Server};
/// use std::net::SocketAddr;
///
/// // Define our initial scope
/// scope_guard!(AdminOnly; "admin");
///
/// // Define an endpoint that will require this scope
/// async fn test_endpoint(_: AdminOnly) -> &'static str {
///     "You're an admin!"
/// }
///
///# #[tokio::main(flavor = "current_thread")] async fn main() {
/// // Build the router
/// let router = Router::new()
///     .route("/test", get(test_endpoint));
///
/// // Construct the server
/// let server = Server::bind(&SocketAddr::new([0, 0, 0, 0].into(), 3000))
///     .serve(router.into_make_service())
///     .await
///     .unwrap();
/// # }
/// ```
///
/// A custom claim type can be used in order to better use the validated data:
///
/// ```
/// use aliri::jwt;
/// use aliri_axum::scope_guard;
/// use aliri_clock::UnixTime;
/// use aliri_oauth2::oauth2;
/// use serde::Deserialize;
///
/// #[derive(Clone, Debug, Deserialize)]
/// pub struct CustomClaims {
///     iss: jwt::Issuer,
///     aud: jwt::Audiences,
///     sub: jwt::Subject,
///     scope: oauth2::Scope,
/// }
///
/// impl jwt::CoreClaims for CustomClaims {
///     fn nbf(&self) -> Option<UnixTime> { None }
///     fn exp(&self) -> Option<UnixTime> { None }
///     fn aud(&self) -> &jwt::Audiences { &self.aud }
///     fn iss(&self) -> Option<&jwt::IssuerRef> { Some(&self.iss) }
///     fn sub(&self) -> Option<&jwt::SubjectRef> { Some(&self.sub) }
/// }
///
/// impl oauth2::HasScope for CustomClaims {
///     fn scope(&self) -> &oauth2::Scope { &self.scope }
/// }
///
/// // Define our initial scope
/// scope_guard!(AdminOnly(CustomClaims); "admin");
///
/// // Define an endpoint that will require this scope
/// async fn test_endpoint(AdminOnly(token): AdminOnly) -> String {
///     format!("Token subject: {}", token.sub)
/// }
///
/// // Or ignore the token if it isn't required
/// async fn test_endpoint_but_ignore_token_payload(_: AdminOnly) -> &'static str {
///     "You're an admin!"
/// }
/// ```
// This would probably work nicer as a procedural macro, as then it could
// produce even better documentation.
#[macro_export]
macro_rules! scope_guard {
    ($vis:vis $i:ident; $scope:literal) => {
        $crate::scope_guard!($vis $i; [$scope]);
    };
    ($vis:vis $i:ident; [$($scope:literal)||* $(,)?]) => {
        $crate::scope_guard!($vis $i(::aliri_oauth2::oauth2::BasicClaimsWithScope); [$($scope)||*]);
    };
    ($vis:vis $i:ident($claim:ty); $scope:literal) => {
        $crate::scope_guard!($vis $i($claim); [$scope]);
    };
    ($vis:vis $i:ident($claim:ty); [$($scope:literal)||* $(,)?]) => {
        #[doc = "Ensures that a claims object authorizes access to a given scope"]
        #[doc = ""]
        #[doc = "The claims object must have one of the following sets of scopes to be considered authorized."]
        #[doc = "Within each set, all scopes must be present, but only one set must be satisfied."]
        #[doc = ""]
        #[doc = "In the event of authorization failures, more verbose messages can be generated by adding "]
        #[doc = "[`aliri_axum::VerboseAuthxErrors`] to the `extensions` of the request."]
        #[doc = ""]
        $(
            #[doc = concat!("* `", $scope, "`")]
        )*
        $vis struct $i($vis $claim);

        impl $i {
            #[allow(dead_code)]
            $vis fn into_claims(self) -> $claim {
                self.0
            }

            #[allow(dead_code)]
            $vis fn claims(&self) -> &$claim {
                &self.0
            }
        }

        impl $crate::EndpointScopePolicy for $i {
            type Claims = $claim;

            fn scope_policy() -> &'static $crate::__private::ScopePolicy {
                static POLICY: $crate::__private::OnceCell<$crate::__private::ScopePolicy> = $crate::__private::OnceCell::new();
                POLICY.get_or_init(|| {
                    $crate::__private::ScopePolicy::deny_all()
                    $(
                        .or_allow($scope.parse().unwrap())
                    )*
                })
            }
        }

        #[::axum::async_trait]
        impl<B> ::axum::extract::FromRequest<B> for $i
        where
            B: Send,
        {
            type Rejection = $crate::AuthFailed;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B>,
            ) -> Result<Self, Self::Rejection> {
                $crate::__private::from_request(req, <Self as $crate::EndpointScopePolicy>::scope_policy()).map(Self)
            }
        }
    };
}

/// Convenience macro for services that need to define many scopes.
///
/// # Example
///
/// ```
/// use aliri_axum::scope_guards;
///
/// scope_guards! {
///     scope AdminOnly = "admin";
///     scope List = "list";
///     scope Read = "read";
///     scope Write = "write";
///     scope ReadWrite = "read write";
///     scope ReadOrList = ["read" || "list"];
/// }
/// ```
///
/// The above will define a scope guard type for each of the scopes, similar to the [`scope_guard!`]
/// macro.
///
/// Using a custom claims type can be done with a `type Claims = <...>` declaration.
///
/// ```
/// use aliri_axum::scope_guards;
/// use aliri_oauth2::oauth2;
///
/// struct CustomClaims {
///     scope: oauth2::Scope,
/// }
///
/// impl oauth2::HasScope for CustomClaims {
///     fn scope(&self) -> &oauth2::Scope {
///        &self.scope
///     }
/// }
///
/// scope_guards! {
///     type Claims = CustomClaims;
///
///     scope AdminOnly = "admin";
///     scope List = "list";
///     scope Read = "read";
///     scope Write = "write";
///     scope ReadWrite = "read write";
///     scope ReadOrList = ["read" || "list"];
/// }
/// ```
#[macro_export]
macro_rules! scope_guards {
    ($($vis:vis scope $i:ident = $scope:tt);* $(;)?) => {
        $(
            $crate::scope_guard!($vis $i; $scope);
        )*
    };
    (type Claims = $claims:ty; $($vis:vis scope $i:ident = $scope:tt);* $(;)?) => {
        $(
            $crate::scope_guard!($vis $i($claims); $scope);
        )*
    };
}

#[cfg(test)]
mod tests {
    use aliri_oauth2::{oauth2, scope};
    use axum::{
        extract::{FromRequest, RequestParts},
        http::Request,
    };

    use crate::AuthFailed;

    scope_guard!(AdminOnly(MyClaims); "admin");

    scope_guards! {
        type Claims = MyClaims;

        scope AdminOnly2 = "admin";
        scope Testing = ["testing" || "testing2"];
        scope TestingAdmin = ["testing admin"];
    }

    struct MyClaims(oauth2::Scope);

    impl oauth2::HasScope for MyClaims {
        fn scope(&self) -> &oauth2::Scope {
            &self.0
        }
    }

    fn request_with_no_claims() -> RequestParts<()> {
        RequestParts::new(Request::new(()))
    }

    fn request_with_scope(scope: oauth2::Scope) -> RequestParts<()> {
        let mut req = RequestParts::new(Request::new(()));
        req.extensions_mut().insert(MyClaims(scope));
        req
    }

    fn request_with_admin_scope() -> RequestParts<()> {
        request_with_scope(scope!["admin"].unwrap())
    }

    fn request_with_no_scope() -> RequestParts<()> {
        request_with_scope(scope![].unwrap())
    }

    fn request_with_testing_scope() -> RequestParts<()> {
        request_with_scope(scope!["testing"].unwrap())
    }

    fn request_with_testing2_scope() -> RequestParts<()> {
        request_with_scope(scope!["testing2"].unwrap())
    }

    fn request_with_admin_and_testing_scope() -> RequestParts<()> {
        request_with_scope(scope!["admin", "testing"].unwrap())
    }

    #[tokio::test]
    async fn admin_only_scope_guard_without_claims_returns_error() {
        match AdminOnly::from_request(&mut request_with_no_claims()).await {
            Err(AuthFailed::MissingClaims) => {}
            Err(AuthFailed::InsufficientScopes { .. }) => panic!("Expected missing claims error"),
            Ok(_) => panic!("Expected AuthFailed"),
        }
    }

    #[tokio::test]
    async fn admin_only_scope_guard_with_admin_scope_claims() {
        AdminOnly::from_request(&mut request_with_admin_scope())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn admin_only_scope_guard_with_admin_and_testing_scope_claims() {
        AdminOnly::from_request(&mut request_with_admin_and_testing_scope())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn admin_only_scope_guard_with_no_scope_claims() {
        match AdminOnly::from_request(&mut request_with_no_scope()).await {
            Err(AuthFailed::InsufficientScopes { .. }) => {}
            Err(AuthFailed::MissingClaims) => panic!("Expected insufficient scopes error"),
            Ok(_) => panic!("Expected AuthFailed"),
        }
    }

    #[tokio::test]
    async fn testing_scope_guard_with_testing_scope_claims() {
        Testing::from_request(&mut request_with_testing_scope())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn testing_scope_guard_with_admin_and_testing_scope_claims() {
        Testing::from_request(&mut request_with_admin_and_testing_scope())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn testing_scope_guard_with_testing2_scope_claims() {
        Testing::from_request(&mut request_with_testing2_scope())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn testing_scope_guard_with_admin_scope_claims() {
        match Testing::from_request(&mut request_with_admin_scope()).await {
            Err(AuthFailed::InsufficientScopes { .. }) => {}
            Err(AuthFailed::MissingClaims) => panic!("Expected insufficient scopes error"),
            Ok(_) => panic!("Expected AuthFailed"),
        }
    }

    #[tokio::test]
    async fn testing_admin_scope_guard_with_testing_scope_claims() {
        match TestingAdmin::from_request(&mut request_with_testing_scope()).await {
            Err(AuthFailed::InsufficientScopes { .. }) => {}
            Err(AuthFailed::MissingClaims) => panic!("Expected insufficient scopes error"),
            Ok(_) => panic!("Expected AuthFailed"),
        }
    }

    #[tokio::test]
    async fn testing_admin_scope_guard_with_admin_scope_claims() {
        match TestingAdmin::from_request(&mut request_with_admin_scope()).await {
            Err(AuthFailed::InsufficientScopes { .. }) => {}
            Err(AuthFailed::MissingClaims) => panic!("Expected insufficient scopes error"),
            Ok(_) => panic!("Expected AuthFailed"),
        }
    }

    #[tokio::test]
    async fn testing_admin_scope_guard_with_admin_and_testing_scope_claims() {
        TestingAdmin::from_request(&mut request_with_admin_and_testing_scope())
            .await
            .unwrap();
    }
}
