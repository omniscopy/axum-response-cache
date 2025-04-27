//! This library provides [Axum middleware](`axum#middleware`) that caches HTTP responses to the
//! incoming requests based on their HTTP method and path.
//!
//! The main struct is [`CacheLayer`]. It can be created with any cache that implements two traits
//! from the [`cached`] crate: [`cached::Cached`] and [`cached::CloneCached`].
//!
//! The *current* version of [`CacheLayer`] is compatible only with services accepting
//! Axum’s [`Request<Body>`](`http::Request<axum::body::Body>`) and returning
//! [`axum::response::Response`], thus it is not compatible with non-Axum [`tower`] services.
//!
//! It’s possible to configure the layer to re-use an old expired response in case the wrapped
//! service fails to produce a new successful response.
//!
//! Only successful responses are cached (responses with status codes outside of the `[200-299]`
//! range are passed-through or ignored).
//!
//! The cache limits maximum size of the response’s body (128 MB by default).
//!
//! ## Examples
//!
//! To cache a response over a specific route, just wrap it in a [`CacheLayer`]:
//!
//! ```rust,no_run
//! # use axum_08 as axum;
//! use axum::{Router, extract::Path, routing::get};
//! use axum_response_cache::CacheLayer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut router = Router::new()
//!         .route(
//!             "/hello/{name}",
//!             get(|Path(name): Path<String>| async move { format!("Hello, {name}!") })
//!                 // this will cache responses with each `:name` for 60 seconds.
//!                 .layer(CacheLayer::with_lifespan(60)),
//!         );
//!
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
//!     axum::serve(listener, router).await.unwrap();
//! }
//! ```
//!
//! ### Reusing last successful response
//!
//! ```rust
//! # use std::sync::atomic::{AtomicBool, Ordering};
//! # use axum_08 as axum;
//! use axum::{
//!     body::Body,
//!     extract::Path,
//!     http::status::StatusCode,
//!     http::Request,
//!     Router,
//!     routing::get,
//! };
//! use axum_response_cache::CacheLayer;
//! use tower::Service as _;
//!
//! // a handler that returns 200 OK only the first time it’s called
//! async fn handler(Path(name): Path<String>) -> (StatusCode, String) {
//!     static FIRST_RUN: AtomicBool = AtomicBool::new(true);
//!     let first_run = FIRST_RUN.swap(false, Ordering::AcqRel);
//!
//!     if first_run {
//!         (StatusCode::OK, format!("Hello, {name}"))
//!     } else {
//!         (StatusCode::INTERNAL_SERVER_ERROR, String::from("Error!"))
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut router = Router::new()
//!     .route("/hello/{name}", get(handler))
//!     .layer(CacheLayer::with_lifespan(60).use_stale_on_failure());
//!
//! // first request will fire handler and get the response
//! let status1 = router.call(Request::get("/hello/foo").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status1);
//!
//! // second request will reuse the last response since the handler now returns ISE
//! let status2 = router.call(Request::get("/hello/foo").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status2);
//! # }
//! ```
//!
//! ### Serving static files
//! This middleware can be used to cache files served in memory to limit hard drive load on the
//! server. To serve files you can use [`tower-http::services::ServeDir`](https://docs.rs/tower-http/latest/tower_http/services/struct.ServeDir.html) layer.
//! ```rust,ignore
//! let router = Router::new().nest_service("/", ServeDir::new("static/"));
//! ```
//!
//! ### Limiting the body size
//!
//! ```rust
//! # use axum_08 as axum;
//! use axum::{
//!     body::Body,
//!     extract::Path,
//!     http::status::StatusCode,
//!     http::Request,
//!     Router,
//!     routing::get,
//! };
//! use axum_response_cache::CacheLayer;
//! use tower::Service as _;
//!
//! // returns a short string, well below the limit
//! async fn ok_handler() -> &'static str {
//!     "ok"
//! }
//!
//! async fn too_long_handler() -> &'static str {
//!     "a response that is well beyond the limit of the cache!"
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut router = Router::new()
//!     .route("/ok", get(ok_handler))
//!     .route("/too_long", get(too_long_handler))
//!     // limit max cached body to only 16 bytes
//!     .layer(CacheLayer::with_lifespan(60).body_limit(16));
//!
//! let status_ok = router.call(Request::get("/ok").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status_ok);
//!
//! let status_too_long = router.call(Request::get("/too_long").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status_too_long);
//! # }
//! ```
//! ### Manual Cache Invalidation
//! This middleware allows manual cache invalidation by setting the `X-Invalidate-Cache` header in the request. This can be useful when you know the underlying data has changed and you want to force a fresh pull of data.
//!
//! ```rust
//! # use axum_08 as axum;
//! use axum::{
//!     body::Body,
//!     extract::Path,
//!     http::status::StatusCode,
//!     http::Request,
//!     Router,
//!     routing::get,
//! };
//! use axum_response_cache::CacheLayer;
//! use tower::Service as _;
//!
//! async fn handler(Path(name): Path<String>) -> (StatusCode, String) {
//!     (StatusCode::OK, format!("Hello, {name}"))
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut router = Router::new()
//!     .route("/hello/{name}", get(handler))
//!     .layer(CacheLayer::with_lifespan(60).allow_invalidation());
//!
//! // first request will fire handler and get the response
//! let status1 = router.call(Request::get("/hello/foo").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status1);
//!
//! // second request should return the cached response
//! let status2 = router.call(Request::get("/hello/foo").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status2);
//!
//! // third request with X-Invalidate-Cache header to invalidate the cache
//! let status3 = router.call(
//!     Request::get("/hello/foo")
//!         .header("X-Invalidate-Cache", "true")
//!         .body(Body::empty())
//!         .unwrap(),
//!     )
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status3);
//!
//! // fourth request to verify that the handler is called again
//! let status4 = router.call(Request::get("/hello/foo").body(Body::empty()).unwrap())
//!     .await
//!     .unwrap()
//!     .status();
//! assert_eq!(StatusCode::OK, status4);
//! # }
//! ```
//! Cache invalidation could be dangerous because it can allow a user to force the server to make a request to an external service or database. It is disabled by default, but can be enabled by calling the [`CacheLayer::allow_invalidation`] method.
//!
//! ## Using custom cache
//! ```rust
//! # use axum_08 as axum;
//! use axum::{Router, routing::get};
//! use axum_response_cache::CacheLayer;
//! // let’s use TimedSizedCache here
//! use cached::stores::TimedSizedCache;
//! # use axum::{body::Body, http::Request};
//! # use tower::ServiceExt;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let router: Router = Router::new()
//!     .route("/hello", get(|| async { "Hello, world!" }))
//!     // cache maximum value of 50 responses for one minute
//!     .layer(CacheLayer::with(TimedSizedCache::with_size_and_lifespan(50, 60)));
//! # // force type inference to resolve the exact type of router
//! #     let _ = router.oneshot(Request::get("/hello").body(Body::empty()).unwrap()).await;
//! # }
//! ```
//!
//! ## Using custom keyer
//! It’s possible to customize the cache’s key to include eg. the `Accept` header (so that
//! different types of responses are cached separately based on the header).
//!
//! ```rust
//! # use axum_08 as axum;
//! use axum::{Router, routing::get};
//! use axum_response_cache::CacheLayer;
//! # use axum::{body::Body, http::Request};
//! # use tower::ServiceExt;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // cache responses based on method, Accept header, and uri
//! let keyer = |request: &Request<Body>| {
//!     (
//!         request.method().clone(),
//!         request
//!             .headers()
//!             .get(axum::http::header::ACCEPT)
//!             .and_then(|c| c.to_str().ok())
//!             .unwrap_or("")
//!             .to_string(),
//!         request.uri().clone(),
//!     )
//! };
//! let router: Router = Router::new()
//!     .route("/hello", get(|| async { "Hello, world!" }))
//!     .layer(CacheLayer::with_lifespan_and_keyer(60, keyer));
//! # // force type inference to resolve the exact type of router
//! #     let _ = router.oneshot(Request::get("/hello").body(Body::empty()).unwrap()).await;
//! # }
//! ```
//!
//! ## Use cases
//! Caching responses in memory (eg. using [`cached::TimedCache`]) might be useful when the
//! underlying service produces the responses by:
//! 1. doing heavy computation,
//! 2. requesting external service(s) that might not be fully reliable or performant,
//! 3. serving static files from disk.
//!
//! In those cases, if the response to identical requests does not change often over time, it might
//! be desirable to re-use the same responses from memory without re-calculating them – skipping requests to data
//! bases, external services, reading from disk.
//!
//! ### Using Axum 0.7
//!
//! By default, this library uses Axum 0.8. However, you can configure it to use Axum 0.7 by enabling the appropriate feature flag in your `Cargo.toml`.
//!
//! To use Axum 0.7, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! axum-response-cache = { version = "0.3", features = ["axum07"], default-features = false }
//! ```
//!
//! This will disable the default Axum 0.8 feature and enable the Axum 0.7 feature instead.

use std::{
    convert::Infallible,
    fmt::Debug,
    future::Future,
    hash::Hash,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tracing_futures::Instrument as _;

#[cfg(feature = "axum07")]
use axum_07 as axum;
#[cfg(feature = "axum08")]
use axum_08 as axum;

use axum::body;
use axum::{
    body::{Body, Bytes},
    http::{response::Parts, Request, StatusCode},
    response::{IntoResponse, Response},
};

use cached::{Cached, CloneCached, TimedCache};
use tower::{Layer, Service};
use tracing::{debug, instrument};

/// The trait for objects used to obtain cache keys. See [`BasicKeyer`] for default implementation
/// returning `(http::Method, Uri)`.
pub trait Keyer {
    type Key;

    fn get_key(&self, request: &Request<Body>) -> Self::Key;
}

impl<K, F> Keyer for F
where
    F: Fn(&Request<Body>) -> K + Send + Sync + 'static,
{
    type Key = K;

    fn get_key(&self, request: &Request<Body>) -> Self::Key {
        self(request)
    }
}

/// The basic caching strategy for the responses.
///
/// The responses are cached according to the HTTP method ([`axum::http::Method`]) and path
/// ([`axum::http::Uri`]) of the request they responded to.
pub struct BasicKeyer;

pub type BasicKey = (http::Method, http::Uri);

impl Keyer for BasicKeyer {
    type Key = BasicKey;

    fn get_key(&self, request: &Request<Body>) -> Self::Key {
        (request.method().clone(), request.uri().clone())
    }
}

/// The struct preserving all the headers and body of the cached response.
#[derive(Clone, Debug)]
pub struct CachedResponse {
    parts: Parts,
    body: Bytes,
    timestamp: Option<std::time::Instant>,
}

impl IntoResponse for CachedResponse {
    fn into_response(self) -> Response {
        let mut response = Response::from_parts(self.parts, Body::from(self.body));
        if let Some(timestamp) = self.timestamp {
            let age = timestamp.elapsed().as_secs();
            response
                .headers_mut()
                .insert("X-Cache-Age", age.to_string().parse().unwrap());
        }
        response
    }
}

/// The main struct of the library. The layer providing caching to the wrapped service.
/// It is generic over the cache used (`C`) and a `Keyer` (`K`) used to obtain the key for cached
/// responses.
pub struct CacheLayer<C, K> {
    cache: Arc<Mutex<C>>,
    use_stale: bool,
    limit: usize,
    allow_invalidation: bool,
    add_response_headers: bool,
    keyer: Arc<K>,
}

impl<C, K> Clone for CacheLayer<C, K> {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            use_stale: self.use_stale,
            limit: self.limit,
            allow_invalidation: self.allow_invalidation,
            add_response_headers: self.add_response_headers,
            keyer: Arc::clone(&self.keyer),
        }
    }
}

impl<C, K> CacheLayer<C, K>
where
    C: Cached<K::Key, CachedResponse> + CloneCached<K::Key, CachedResponse>,
    K: Keyer,
    K::Key: Debug + Hash + Eq + Clone + Send + 'static,
{
    /// Create a new cache layer with a given cache and the default body size limit of 128 MB.
    pub fn with_cache_and_keyer(cache: C, keyer: K) -> Self {
        Self {
            cache: Arc::new(Mutex::new(cache)),
            use_stale: false,
            limit: 128 * 1024 * 1024,
            allow_invalidation: false,
            add_response_headers: false,
            keyer: Arc::new(keyer),
        }
    }

    /// Switch the layer’s settings to preserve the last successful response even when it’s evicted
    /// from the cache but the service failed to provide a new successful response (ie. eg. when
    /// the underlying service responds with `404 NOT FOUND`, the cache will keep providing the last stale `200 OK`
    /// response produced).
    pub fn use_stale_on_failure(self) -> Self {
        Self {
            use_stale: true,
            ..self
        }
    }

    /// Change the maximum body size limit. If you want unlimited size, use [`usize::MAX`].
    pub fn body_limit(self, new_limit: usize) -> Self {
        Self {
            limit: new_limit,
            ..self
        }
    }

    /// Allow manual cache invalidation by setting the `X-Invalidate-Cache` header in the request.
    /// This will allow the cache to be invalidated for the given key.
    pub fn allow_invalidation(self) -> Self {
        Self {
            allow_invalidation: true,
            ..self
        }
    }

    /// Allow the response headers to be included in the cached response.
    pub fn add_response_headers(self) -> Self {
        Self {
            add_response_headers: true,
            ..self
        }
    }
}

impl<C> CacheLayer<C, BasicKeyer>
where
    C: Cached<BasicKey, CachedResponse> + CloneCached<BasicKey, CachedResponse>,
{
    /// Create a new cache layer with a given cache and the default body size limit of 128 MB.
    pub fn with(cache: C) -> Self {
        Self {
            cache: Arc::new(Mutex::new(cache)),
            use_stale: false,
            limit: 128 * 1024 * 1024,
            allow_invalidation: false,
            add_response_headers: false,
            keyer: Arc::new(BasicKeyer),
        }
    }
}

impl CacheLayer<TimedCache<BasicKey, CachedResponse>, BasicKey> {
    /// Create a new cache layer with the desired TTL in seconds
    pub fn with_lifespan(
        ttl_sec: u64,
    ) -> CacheLayer<TimedCache<BasicKey, CachedResponse>, BasicKeyer> {
        CacheLayer::with(TimedCache::with_lifespan(ttl_sec))
    }
}

impl<K> CacheLayer<TimedCache<K::Key, CachedResponse>, K>
where
    K: Keyer,
    K::Key: Debug + Hash + Eq + Clone + Send + 'static,
{
    /// Create a new cache layer with the desired TTL in seconds
    pub fn with_lifespan_and_keyer(
        ttl_sec: u64,
        keyer: K,
    ) -> CacheLayer<TimedCache<K::Key, CachedResponse>, K> {
        CacheLayer::with_cache_and_keyer(TimedCache::with_lifespan(ttl_sec), keyer)
    }
}

impl<S, C, K> Layer<S> for CacheLayer<C, K>
where
    K: Keyer,
    K::Key: Debug + Hash + Eq + Clone + Send + 'static,
{
    type Service = CacheService<S, C, K>;

    fn layer(&self, inner: S) -> Self::Service {
        Self::Service {
            inner,
            cache: Arc::clone(&self.cache),
            use_stale: self.use_stale,
            limit: self.limit,
            allow_invalidation: self.allow_invalidation,
            add_response_headers: self.add_response_headers,
            keyer: Arc::clone(&self.keyer),
        }
    }
}

pub struct CacheService<S, C, K> {
    inner: S,
    cache: Arc<Mutex<C>>,
    use_stale: bool,
    limit: usize,
    allow_invalidation: bool,
    add_response_headers: bool,
    keyer: Arc<K>,
}

impl<S, C, K> Clone for CacheService<S, C, K>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            cache: Arc::clone(&self.cache),
            use_stale: self.use_stale,
            limit: self.limit,
            allow_invalidation: self.allow_invalidation,
            add_response_headers: self.add_response_headers,
            keyer: Arc::clone(&self.keyer),
        }
    }
}

impl<S, C, K> Service<Request<Body>> for CacheService<S, C, K>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send,
    S::Future: Send + 'static,
    C: Cached<K::Key, CachedResponse> + CloneCached<K::Key, CachedResponse> + Send + 'static,
    K: Keyer,
    K::Key: Debug + Hash + Eq + Clone + Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[instrument(skip(self, request))]
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let use_stale = self.use_stale;
        let allow_invalidation = self.allow_invalidation;
        let add_response_headers = self.add_response_headers;
        let limit = self.limit;
        let cache = Arc::clone(&self.cache);
        let key = self.keyer.get_key(&request);

        // Check for the custom header "X-Invalidate-Cache" if invalidation is allowed
        if allow_invalidation && request.headers().contains_key("X-Invalidate-Cache") {
            // Manually invalidate the cache for this key
            cache.lock().unwrap().cache_remove(&key);
            debug!("Cache invalidated manually for key {:?}", key);
        }

        let inner_fut = inner
            .call(request)
            .instrument(tracing::info_span!("inner_service"));
        let (cached, evicted) = {
            let mut guard = cache.lock().unwrap();
            let (cached, evicted) = guard.cache_get_expired(&key);
            if let (Some(stale), true) = (cached.as_ref(), evicted) {
                // reinsert stale value immediately so that others don’t schedule their updating
                debug!("Found stale value in cache, reinsterting and attempting refresh");
                guard.cache_set(key.clone(), stale.clone());
            }
            (cached, evicted)
        };

        Box::pin(async move {
            match (cached, evicted) {
                (Some(value), false) => Ok(value.into_response()),
                (Some(stale_value), true) => {
                    let response = inner_fut.await.unwrap();
                    if response.status().is_success() {
                        Ok(update_cache(&cache, key, response, limit, add_response_headers).await)
                    } else if use_stale {
                        debug!("Returning stale value.");
                        Ok(stale_value.into_response())
                    } else {
                        debug!("Stale value in cache, evicting and returning failed response.");
                        cache.lock().unwrap().cache_remove(&key);
                        Ok(response)
                    }
                }
                (None, _) => {
                    let response = inner_fut.await.unwrap();
                    if response.status().is_success() {
                        Ok(update_cache(&cache, key, response, limit, add_response_headers).await)
                    } else {
                        Ok(response)
                    }
                }
            }
        })
    }
}

#[instrument(skip(cache, response))]
async fn update_cache<C, K>(
    cache: &Arc<Mutex<C>>,
    key: K,
    response: Response,
    limit: usize,
    add_response_headers: bool,
) -> Response
where
    C: Cached<K, CachedResponse> + CloneCached<K, CachedResponse>,
    K: Debug + Hash + Eq + Clone + Send + 'static,
{
    let (parts, body) = response.into_parts();
    let Ok(body) = body::to_bytes(body, limit).await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("File too big, over {limit} bytes"),
        )
            .into_response();
    };
    let value = CachedResponse {
        parts,
        body,
        timestamp: if add_response_headers {
            Some(std::time::Instant::now())
        } else {
            None
        },
    };
    {
        cache.lock().unwrap().cache_set(key, value.clone());
    }
    value.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::sync::atomic::{AtomicIsize, Ordering};

    #[cfg(feature = "axum07")]
    use axum_07 as axum;
    #[cfg(feature = "axum08")]
    use axum_08 as axum;

    use axum::{
        extract::State,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };

    use tower::Service;

    #[derive(Clone, Debug)]
    struct Counter {
        value: Arc<AtomicIsize>,
    }

    impl Counter {
        fn new(init: isize) -> Self {
            Self {
                value: AtomicIsize::from(init).into(),
            }
        }

        fn increment(&self) {
            self.value.fetch_add(1, Ordering::Release);
        }

        fn read(&self) -> isize {
            self.value.load(Ordering::Acquire)
        }
    }

    #[tokio::test]
    async fn should_use_cached_value() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60).use_stale_on_failure();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        for _ in 0..10 {
            let status = router
                .call(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status();
            assert!(status.is_success(), "handler should return success");
        }

        assert_eq!(1, counter.read(), "handler should’ve been called only once");
    }

    #[tokio::test]
    async fn should_not_cache_unsuccessful_responses() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            let responses = [
                StatusCode::BAD_REQUEST,
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::NOT_FOUND,
            ];
            let mut rng = rand::rng();
            responses[rng.random_range(0..responses.len())]
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60).use_stale_on_failure();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        for _ in 0..10 {
            let status = router
                .call(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status();
            assert!(!status.is_success(), "handler should never return success");
        }

        assert_eq!(
            10,
            counter.read(),
            "handler should’ve been called for all requests"
        );
    }

    #[tokio::test]
    async fn should_use_last_correct_stale_value() {
        let handler = |State(cnt): State<Counter>| async move {
            let prev = cnt.value.fetch_add(1, Ordering::AcqRel);
            let responses = [
                StatusCode::BAD_REQUEST,
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::NOT_FOUND,
            ];
            let mut rng = rand::rng();

            // first response successful, later failed
            if prev == 0 {
                StatusCode::OK
            } else {
                responses[rng.random_range(0..responses.len())]
            }
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(1).use_stale_on_failure();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter);

        // feed the cache
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // wait over 1s for cache eviction
        tokio::time::sleep(tokio::time::Duration::from_millis(1050)).await;

        for _ in 1..10 {
            let status = router
                .call(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status();
            assert!(
                status.is_success(),
                "cache should return stale successful value"
            );
        }
    }

    #[tokio::test]
    async fn should_not_use_stale_values() {
        let handler = |State(cnt): State<Counter>| async move {
            let prev = cnt.value.fetch_add(1, Ordering::AcqRel);
            let responses = [
                StatusCode::BAD_REQUEST,
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::NOT_FOUND,
            ];
            let mut rng = rand::rng();

            // first response successful, later failed
            if prev == 0 {
                StatusCode::OK
            } else {
                responses[rng.random_range(0..responses.len())]
            }
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(1);
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // feed the cache
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // wait over 1s for cache eviction
        tokio::time::sleep(tokio::time::Duration::from_millis(1050)).await;

        for _ in 1..10 {
            let status = router
                .call(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status();
            assert!(
                !status.is_success(),
                "cache should forward unsuccessful values"
            );
        }

        assert_eq!(
            10,
            counter.read(),
            "handler should’ve been called for all requests"
        );
    }

    #[tokio::test]
    async fn should_not_invalidate_cache_when_disabled() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60);
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // First request to cache the response
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Second request should return the cached response - no increment
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Third request with X-Invalidate-Cache header should not invalidate the cache - no increment
        let status = router
            .call(
                Request::get("/")
                    .header("X-Invalidate-Cache", "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Fourth request should still return the cached response - no increment
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        assert_eq!(1, counter.read(), "handler should’ve been called only once");
    }

    #[tokio::test]
    async fn should_invalidate_cache_when_enabled() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60).allow_invalidation();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // First request to cache the response
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Second request should return the cached response - no increment
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Third request with X-Invalidate-Cache header to invalidate the cache
        let status = router
            .call(
                Request::get("/")
                    .header("X-Invalidate-Cache", "true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        // Fourth request to verify that the handler is called again
        let status = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "handler should return success");

        assert_eq!(2, counter.read(), "handler should’ve been called twice");
    }

    #[tokio::test]
    async fn should_not_include_age_header_when_disabled() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60);
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // First request to cache the response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "handler should return success"
        );

        // Second request should return the cached response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "handler should return success"
        );
        assert!(
            response.headers().get("X-Cache-Age").is_none(),
            "Age header should not be present"
        );

        assert_eq!(1, counter.read(), "handler should’ve been called only once");
    }

    #[tokio::test]
    async fn should_include_age_header_when_enabled() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let cache = CacheLayer::with_lifespan(60).add_response_headers();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // First request to cache the response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "handler should return success"
        );

        // Age should be 0
        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "0",
            "Age header should be present and equal to 0"
        );
        // wait over 2s to age the cache
        tokio::time::sleep(tokio::time::Duration::from_millis(2100)).await;
        // Second request should return the cached response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "2",
            "Age header should be present and equal to 2"
        );

        assert_eq!(1, counter.read(), "handler should’ve been called only once");
    }

    #[tokio::test]
    async fn should_cache_by_custom_keys() {
        let handler = |State(cnt): State<Counter>| async move {
            cnt.increment();
            StatusCode::OK
        };

        let counter = Counter::new(0);
        let keyer = |request: &Request<Body>| {
            (
                request.method().clone(),
                request
                    .headers()
                    .get(axum::http::header::ACCEPT)
                    .and_then(|c| c.to_str().ok())
                    .unwrap_or("")
                    .to_string(),
                request.uri().clone(),
            )
        };
        let cache = CacheLayer::with_lifespan_and_keyer(60, keyer).add_response_headers();
        let mut router = Router::new()
            .route("/", get(handler).layer(cache))
            .with_state(counter.clone());

        // First request to cache the response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "handler should return success"
        );

        // Age should be 0
        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "0",
            "Age header should be present and equal to 0"
        );

        // wait over 2s to age the cache
        tokio::time::sleep(tokio::time::Duration::from_millis(2100)).await;
        // Second request should return the cached response
        let response = router
            .call(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "2",
            "Age header should be present and equal to 2"
        );

        // Request with a different accept header should return a new response
        let response = router
            .call(
                Request::get("/")
                    .header(axum::http::header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "0",
            "Age header should be present and equal to 0"
        );

        // wait over 2s to age the cache
        tokio::time::sleep(tokio::time::Duration::from_millis(2100)).await;
        // Second request should return the newly cached response
        let response = router
            .call(
                Request::get("/")
                    .header(axum::http::header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get("X-Cache-Age")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
            "2",
            "Age header should be present and equal to 2"
        );

        assert_eq!(
            2,
            counter.read(),
            "handler should’ve been called only twice"
        );
    }
}
