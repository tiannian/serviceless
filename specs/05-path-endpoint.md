# Path Endpoint

## Overview

This document describes the implementation of `PathEndpoint`, a specialized endpoint implementation that provides URI path segment filtering capabilities. The `PathEndpoint` acts as a middleware-like filter that checks the path segments of incoming requests and either forwards matching requests to downstream services or returns error responses for non-matching requests.

## Core Concepts

### PathEndpoint

The `PathEndpoint` filters requests based on URI path segments:

- **Purpose**: Ensures only requests matching a specific path segment are allowed to pass through
- **Segment-Based Matching**: Matches only the first segment of the request path, not the entire path
- **Path Decomposition**: To match a path like `/a/b/c`, multiple `PathEndpoint` instances are needed (one for `a`, one for `b`, one for `c`)
- **Catch-All Support**: Provides a catch-all mode that matches any remaining path segments
- **Forwarding**: Forwards matching requests to the first service in `next` with the matched segment removed from the path
- **Error Handling**: Returns `404 Not Found` for non-matching paths

## Design Principles

### PathEndpoint Implementation

The `PathEndpoint` filters requests by matching a single path segment:

```rust
pub struct PathEndpoint {
    segment: Option<String>,
    catch_all: bool,
}

impl PathEndpoint {
    /// Create a new PathEndpoint that matches a specific path segment
    pub fn new(segment: impl Into<String>) -> Self {
        Self {
            segment: Some(segment.into()),
            catch_all: false,
        }
    }
    
    /// Create a catch-all PathEndpoint that matches any remaining path segments
    pub fn catch_all() -> Self {
        Self {
            segment: None,
            catch_all: true,
        }
    }
    
    /// Extract the first segment from a path
    fn extract_first_segment(path: &str) -> Option<&str> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return None;
        }
        path.split('/').next()
    }
    
    /// Remove the first segment from a path
    fn remove_first_segment(path: &str) -> String {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return String::from("/");
        }
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() <= 1 {
            String::from("/")
        } else {
            format!("/{}", segments[1..].join("/"))
        }
    }
    
    /// Check if the first segment of the request path matches
    fn matches(&self, request_path: &str) -> bool {
        if self.catch_all {
            // Catch-all matches any non-empty path
            !request_path.trim_start_matches('/').is_empty()
        } else if let Some(ref segment) = self.segment {
            // Match the first segment
            Self::extract_first_segment(request_path)
                .map(|first| first == segment.as_str())
                .unwrap_or(false)
        } else {
            false
        }
    }
}

#[async_trait]
impl Endpoint for PathEndpoint {
    async fn route(
        &self,
        request: &Request,
        next: &[Address<HttpService>],
    ) -> Option<&Address<HttpService>> {
        let request_path = request.uri().path();
        
        // Check if the first segment matches
        if self.matches(request_path) {
            // Path segment matches, forward to the first next service
            // Note: The actual path modification should be handled by HttpService
            // when forwarding the request, removing the matched segment
            next.first()
        } else {
            // Path doesn't match, handle locally (will return 404)
            None
        }
    }
    
    async fn handle_leaf(&self, request: &Request) -> Response {
        let request_path = request.uri().path();
        let expected = if self.catch_all {
            "any remaining path segments"
        } else if let Some(ref segment) = self.segment {
            format!("path segment: {}", segment)
        } else {
            "a path segment".to_string()
        };
        
        // Return 404 Not Found for non-matching paths
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::from(format!(
                "Path not found: {}. Expected {}",
                request_path,
                expected
            )))
            .unwrap()
    }
}
```

Key characteristics:
- **Segment-Based Matching**: Matches only the first segment of the request path
- **Single Segment**: Each `PathEndpoint` matches exactly one path segment
- **Path Decomposition**: Multiple `PathEndpoint` instances are chained to match multi-segment paths
- **Catch-All Mode**: `catch_all()` creates an endpoint that matches any remaining path segments
- **Path Modification**: When forwarding, the matched segment should be removed from the request path
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `404 Not Found` for non-matching paths

**Note on Path Modification**: The `route()` method returns `Some(&Address)` when a segment matches, but the actual path modification (removing the matched segment) should be handled by `HttpService` when forwarding the request. This can be implemented by:
- Modifying the request URI before forwarding
- Using request extensions to track path segments
- Creating a new request with the modified path

The `remove_first_segment()` helper method is provided to assist with path modification, but the actual implementation depends on how `HttpService` handles request forwarding.

## Architecture

### Request Flow with PathEndpoint

1. HTTP request arrives at `HttpService` with `PathEndpoint`
2. `PathEndpoint::route()` extracts the first segment from the request path
3. `PathEndpoint::route()` checks if the first segment matches the configured segment (or catch-all)
4. If segment matches:
   - Returns `Some(&Address)` pointing to the first service in `next`
   - The matched segment is removed from the request path
   - Request is forwarded to that service with the modified path
5. If segment doesn't match:
   - Returns `None`
   - `handle_leaf()` is called, returning `404 Not Found`

### Path Segment Processing

When a `PathEndpoint` matches and forwards a request:

- **Segment Extraction**: The first segment is extracted from the request path (e.g., `/a/b/c` → `a`)
- **Segment Matching**: The extracted segment is compared with the endpoint's configured segment
- **Path Modification**: If matched, the segment is removed from the path (e.g., `/a/b/c` → `/b/c`)
- **Forwarding**: The modified request is forwarded to the next service
- **Catch-All**: If using `catch_all()`, all remaining segments are consumed and the path becomes `/`

### Composition Pattern

`PathEndpoint` is designed to be composed with other endpoints:

```
HttpService (MethodEndpoint::new().get().post())
└── next: [Address<HttpService>]
    └── HttpService (PathEndpoint::new("api"))
        └── next: [Address<HttpService>]
            └── HttpService (PathEndpoint::new("users"))
                └── next: [Address<HttpService>]
                    └── HttpService (BusinessLogicEndpoint)
```

In this example:
1. `MethodEndpoint` filters for GET and POST requests (using builder pattern)
2. First `PathEndpoint` matches the `api` segment (`/api/users` → `/users`)
3. Second `PathEndpoint` matches the `users` segment (`/users` → `/`)
4. Only GET and POST requests to `/api/users` reach the business logic endpoint

### Multi-Segment Path Matching

To match a path like `/a/b/c`, you need three `PathEndpoint` instances:

```
HttpService (PathEndpoint::new("a"))
└── next: [Address<HttpService>]
    └── HttpService (PathEndpoint::new("b"))
        └── next: [Address<HttpService>]
            └── HttpService (PathEndpoint::new("c"))
                └── next: [Address<HttpService>]
                    └── HttpService (HandlerEndpoint)
```

Request flow:
- Request `/a/b/c` arrives at first `PathEndpoint`
- First segment `a` matches, path becomes `/b/c`, forwarded to second `PathEndpoint`
- Second segment `b` matches, path becomes `/c`, forwarded to third `PathEndpoint`
- Third segment `c` matches, path becomes `/`, forwarded to handler

## Design Decisions

### Why Return None for Non-Matching Requests?

`PathEndpoint` returns `None` from `route()` for non-matching requests:

- **Clear Semantics**: `None` clearly indicates the request should be handled locally
- **Error Handling**: Allows `handle_leaf()` to return appropriate error responses
- **Separation of Concerns**: Filtering logic is separate from error response generation
- **Consistency**: Follows the same pattern as other endpoints that handle requests locally

### Why Forward to First Service Only?

`PathEndpoint` forwards to `next.first()`:

- **Simplicity**: Keeps the filtering logic simple and predictable
- **Single Target**: Filter endpoints typically have one downstream service
- **Composability**: Multiple filters can be chained to achieve complex routing
- **Flexibility**: More complex routing can be handled by dedicated router endpoints

### Why Segment-Based Matching Instead of Full Path Matching?

`PathEndpoint` matches only a single path segment instead of the full path:

- **Composability**: Enables building complex routes by composing multiple `PathEndpoint` instances
- **Modularity**: Each endpoint handles one segment, making the routing logic clear and testable
- **Flexibility**: Easy to create different route combinations by chaining different segments
- **Path Decomposition**: Naturally decomposes paths into segments, matching how URLs are structured
- **Reusability**: Individual segment endpoints can be reused in different route configurations
- **Explicit Routing**: Makes it explicit which segment is being matched at each level

### Why Support Catch-All?

`PathEndpoint` provides a `catch_all()` mode:

- **Flexible Routing**: Allows matching any remaining path segments after specific segments
- **Wildcard Behavior**: Useful for routes that need to accept variable path segments
- **Simplified Handling**: Avoids needing to create endpoints for every possible path segment
- **Common Pattern**: Matches common web framework patterns (e.g., `/api/*` or `/static/*`)

## Examples

### Basic PathEndpoint Usage

```rust
// Create a service that matches the "api" segment
let api_endpoint = PathEndpoint::new("api");
let api_service = HttpService::new(api_endpoint)
    .append_next(api_handler_addr);

let (addr, future) = api_service.start();
tokio::spawn(future);

// Request to /api - first segment "api" matches, path becomes "/"
let api_request = Request::builder()
    .uri("/api")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: api_request }).await?;
// Response comes from api_handler service

// Request to /static - first segment "static" doesn't match "api", returns 404
let static_request = Request::builder()
    .uri("/static")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: static_request }).await?;
// Response is 404 Not Found

// Using catch-all to match any remaining segments
let catch_all_endpoint = PathEndpoint::catch_all();
let catch_all_service = HttpService::new(catch_all_endpoint)
    .append_next(handler_addr);

let (catch_all_addr, catch_all_future) = catch_all_service.start();
tokio::spawn(catch_all_future);

// Request to /anything/here/will/match - catch-all matches all segments
let any_request = Request::builder()
    .uri("/anything/here/will/match")
    .body(BoxBody::empty())
    .unwrap();
let response = catch_all_addr.call(HttpRequest { request: any_request }).await?;
// Response comes from handler service
```

### Multi-Segment Path Matching

```rust
// Create endpoint chain for /api/v1/users path
let users_handler = HttpService::new(UsersHandlerEndpoint);
let (users_handler_addr, users_handler_future) = users_handler.start();
tokio::spawn(users_handler_future);

// Third segment: "users"
let users_endpoint = HttpService::new(PathEndpoint::new("users"))
    .append_next(users_handler_addr.into());
let (users_addr, users_future) = users_endpoint.start();
tokio::spawn(users_future);

// Second segment: "v1"
let v1_endpoint = HttpService::new(PathEndpoint::new("v1"))
    .append_next(users_addr.into());
let (v1_addr, v1_future) = v1_endpoint.start();
tokio::spawn(v1_future);

// First segment: "api"
let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(v1_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Request to /api/v1/users - matches all segments
let users_request = Request::builder()
    .uri("/api/v1/users")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: users_request }).await?;
// Request flow: /api/v1/users -> "api" -> /v1/users -> "v1" -> /users -> "users" -> /
// Forwarded to handler

// Request to /api/v1/posts - "users" segment doesn't match, returns 404
let posts_request = Request::builder()
    .uri("/api/v1/posts")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: posts_request }).await?;
// Returns 404 Not Found at the "users" endpoint level
```

### Catch-All Usage

```rust
// Create endpoint chain: api -> catch-all
let handler = HttpService::new(ApiHandlerEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

let catch_all_endpoint = HttpService::new(PathEndpoint::catch_all())
    .append_next(handler_addr.into());
let (catch_all_addr, catch_all_future) = catch_all_endpoint.start();
tokio::spawn(catch_all_future);

let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(catch_all_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Request to /api/anything/here - "api" matches, catch-all matches the rest
let any_request = Request::builder()
    .uri("/api/anything/here")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: any_request }).await?;
// Request flow: /api/anything/here -> "api" -> /anything/here -> catch-all -> /
// Forwarded to handler
```

### Composing with Other Endpoints

```rust
// Create handler service
let handler = HttpService::new(BusinessLogicEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

// Create path filter chain: api -> users
let users_endpoint = HttpService::new(PathEndpoint::new("users"))
    .append_next(handler_addr.into());
let (users_addr, users_future) = users_endpoint.start();
tokio::spawn(users_future);

let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(users_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Create method filter (wraps path filter) - allows GET and POST
let method_filter = HttpService::new(MethodEndpoint::new().get().post())
    .append_next(api_addr.into());
let (root_addr, root_future) = method_filter.start();
tokio::spawn(root_future);

// Only GET and POST requests to /api/users will reach the handler
// Request flow: /api/users -> method check -> /api/users -> "api" segment -> /users -> "users" segment -> /
let get_request = Request::builder()
    .method(Method::GET)
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();
let response = root_addr.call(HttpRequest { request: get_request }).await?;

let post_request = Request::builder()
    .method(Method::POST)
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();
let response = root_addr.call(HttpRequest { request: post_request }).await?;
```

## Integration with Actor Model

### PathEndpoint as Actor

`PathEndpoint` participates in the actor system through `HttpService`:

- **Service Trait**: `HttpService` with `PathEndpoint` implements `Service` trait
- **Message Handling**: Handles `HttpRequest` messages through the `Handler` trait
- **Async Processing**: All filtering and forwarding operations are asynchronous
- **Address**: Can be addressed via `Address<HttpService>`

### Request Flow Through PathEndpoint

1. HTTP request arrives as `HttpRequest` message
2. `HttpService` calls `endpoint.route()` with the request
3. `PathEndpoint` extracts first segment from path and checks if it matches the configured segment (or catch-all)
4. If match:
   - The matched segment is removed from the request path
   - Request is forwarded to next service via `Address::call()` with modified request
5. If no match: `handle_leaf()` generates error response
6. Response flows back through the service chain

## Future Considerations

### PathEndpoint Enhancements

- **Regex Segment Matching**: Support for regex-based segment patterns (e.g., match segments starting with "user")
- **Path Parameters**: Extract path parameters from matching segments (e.g., `:id` pattern)
- **Case Sensitivity**: Configurable case sensitivity for segment matching
- **Trailing Slash Handling**: Configurable handling of trailing slashes in paths
- **Path Normalization**: Normalize paths before segment extraction (e.g., remove duplicate slashes)
- **Performance**: Consider using `&str` instead of `String` for segment storage to avoid allocations
- **Segment Validation**: Support for validating segment format (e.g., numeric IDs, UUIDs)

### General Enhancements

- **Custom Error Responses**: Allow customization of error response bodies
- **Filter Composition**: Helper types for composing multiple filters more easily (e.g., `PathChain` builder)
- **Path Modification API**: Standardized way to modify request paths when forwarding (e.g., via Request extensions)
- **Segment Extraction Utilities**: Helper functions for common path manipulation operations
