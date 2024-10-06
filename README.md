This library provides Axum middleware that caches HTTP responses to the
incoming requests based on their HTTP method and path.

The main struct is `CacheLayer`. It can be created with any cache that implements two traits
from the `cached` crate: `cached::Cached` and `cached::CloneCached`.

The *current* version of `CacheLayer` is compatible only with services accepting
Axum’s [`Request<Body>`](`http::Request<axum::body::Body>`) and returning
`axum::response::Response`, thus it is not compatible with non-Axum `tower` services.

It’s possible to configure the layer to re-use an old expired response in case the wrapped
service fails to produce a new successful response.

Only successful responses are cached (responses with status codes outside of the `[200-299]`
range are passed-through or ignored).

The cache limits maximum size of the response’s body (128 MB by default).

## Example

To cache a response over a specific route, just wrap it in a [`CacheLayer`]:

```rust
use axum::{Router, extract::Path, routing::get};
use axum_response_cache::CacheLayer;

#[tokio::main]
async fn main() {
    let mut router = Router::new()
        .route(
            "/hello/:name",
            get(|Path(name): Path<String>| async move { format!("Hello, {name}!") })
                // this will cache responses with each `:name` for 60 seconds.
                .layer(CacheLayer::with_lifespan(60)),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
```

For more see documentation.
