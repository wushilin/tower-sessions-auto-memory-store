# tower-sessions-auto-memory-store
A Tower Session store that is in memory, and auto evict old sessions.

# Why?
The default memory store has a few limitation:

1. Deleting expired session requires custom code. This implementation is fully automated.
2. The deletion/update is using Mutex. This code changed to RWLock, which supposed to be more performant
3. Deleting expred session is more performant. This is implemented using VecDequeue and HashMap, so delete can be very efficient


# This should be a drop in replacement for the memory store.

Usage is very simple:

```rust
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use tower_sessions::{cookie::time::{self, Duration, OffsetDateTime}, Expiry, Session, SessionManagerLayer};

#[tokio::main]
async fn main() {
  let session_store = tower_sessions_auto_memory_store::ExpiringMemoryStore::new();
  let session_layer = SessionManagerLayer::new(session_store)
     .with_secure(false)
     .with_expiry(Expiry::OnInactivity(Duration::seconds(1800)));
  let app = Router::new()
     .route("/debug/test", get(root))
     .layer(session_layer);
   let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
   println!("listening on {}", addr);
   axum_server::bind(addr)
     .serve(app.into_make_service())
     .await
     .unwrap();
}

async fn root() -> &'static str {
  "Hello, World!"
}
```
# When the expired session is deleted?
Basically 2 conditions:
1. Every 5 seconds, deletion is checked in the order of creation.
2. The session is expired based on the criterial you defined + 60 seconds to prevent race condition.

# Enjoy
